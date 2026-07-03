#!/usr/bin/env bash
# Driver for the per-binding boundary benchmark harness.
#
# Builds the prerequisites for the requested runtimes, runs each runner
# across all three workloads, and collects the emitted JSON lines into
# output/boundary-<timestamp>.jsonl. Render tables from that file with:
#   python3 render.py output/boundary-<timestamp>.jsonl
#
# Usage:
#   ./run.sh                 # the default (toolchain-light) five
#   ./run.sh c-abi node      # a subset
#   ./run.sh all             # all nine runtimes
#
# Default runtimes:  rust-core c-abi node python wasm — need only the
#   toolchains the compare harness already uses.
# Extended runtimes: go dotnet jvm php — need their language toolchains;
#   jvm additionally needs a real JDK on PATH/JAVA_HOME (the macOS
#   system `java` stub has no runtime).
set -euo pipefail

BOUNDARY_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$BOUNDARY_DIR/../../.." && pwd)"
WORKLOADS="$BOUNDARY_DIR/workloads"
OUT_DIR="$BOUNDARY_DIR/output"
BUILD_DIR="$OUT_DIR/build"

DEFAULT_RUNTIMES="rust-core c-abi node python wasm"
EXTENDED_RUNTIMES="go dotnet jvm php"

if [ "$#" -eq 0 ]; then
  RUNTIMES="$DEFAULT_RUNTIMES"
elif [ "$1" = "all" ]; then
  RUNTIMES="$DEFAULT_RUNTIMES $EXTENDED_RUNTIMES"
else
  RUNTIMES="$*"
fi

mkdir -p "$OUT_DIR" "$BUILD_DIR"
OUT_FILE="$OUT_DIR/boundary-$(date +%s).jsonl"

# Workloads are checked in and byte-stable; refuse to run on drift.
python3 "$WORKLOADS/generate.py" --check

# The dylib/so search path for runners that link libdatalogic_c.
C_LIB_DIR="$REPO_ROOT/bindings/c/target/release"
export DYLD_LIBRARY_PATH="$C_LIB_DIR:${DYLD_LIBRARY_PATH:-}"
export LD_LIBRARY_PATH="$C_LIB_DIR:${LD_LIBRARY_PATH:-}"

note() { printf '\n== %s ==\n' "$*" >&2; }

for rt in $RUNTIMES; do
  case "$rt" in
    rust-core)
      note "rust-core: cargo build + run"
      # Build from the repo root so the benchmark crate's
      # `-C target-cpu=native` config (cwd-scoped) does NOT apply —
      # portable numbers, same convention as the compare harness docs.
      (cd "$REPO_ROOT" && cargo build --release -p datalogic-bench --bin boundary_core)
      "$REPO_ROOT/target/release/boundary_core" "$WORKLOADS" | tee -a "$OUT_FILE"
      ;;
    c-abi)
      note "c-abi: cargo build + cc + run"
      (cd "$REPO_ROOT/bindings/c" && cargo build --release)
      cc -O2 -o "$BUILD_DIR/runner-c" "$BOUNDARY_DIR/runner-c.c" \
        -I "$REPO_ROOT/bindings/c/include" -L "$C_LIB_DIR" -ldatalogic_c
      "$BUILD_DIR/runner-c" "$WORKLOADS" | tee -a "$OUT_FILE"
      ;;
    node)
      note "node: napi build + run"
      (cd "$REPO_ROOT/bindings/node" \
        && { [ -d node_modules ] || npm install --no-audit --no-fund; } \
        && npx napi build --platform --release)
      node "$BOUNDARY_DIR/runner-node.mjs" "$WORKLOADS" | tee -a "$OUT_FILE"
      ;;
    python)
      note "python: maturin build + venv install + run"
      if ! command -v maturin >/dev/null 2>&1; then
        echo "maturin not found; installing with pip install maturin --user" >&2
        pip install maturin --user
      fi
      (cd "$REPO_ROOT/bindings/python" && maturin build --release)
      [ -d "$BOUNDARY_DIR/.venv" ] || python3 -m venv "$BOUNDARY_DIR/.venv"
      # Newest wheel only — older versions may coexist in target/wheels.
      WHEEL="$(ls -t "$REPO_ROOT"/bindings/python/target/wheels/*.whl | head -1)"
      "$BOUNDARY_DIR/.venv/bin/pip" install --quiet --force-reinstall "$WHEEL"
      "$BOUNDARY_DIR/.venv/bin/python" "$BOUNDARY_DIR/runner-python.py" "$WORKLOADS" | tee -a "$OUT_FILE"
      ;;
    wasm)
      note "wasm: build.sh (if pkg missing) + run"
      # build.sh takes minutes (3 targets + wasm-opt); reuse an existing
      # pkg unless it's absent or BOUNDARY_REBUILD_WASM=1.
      if [ ! -f "$REPO_ROOT/bindings/wasm/pkg/nodejs/datalogic_wasm.js" ] \
         || [ "${BOUNDARY_REBUILD_WASM:-0}" = "1" ]; then
        (cd "$REPO_ROOT/bindings/wasm" && ./build.sh)
      fi
      node "$BOUNDARY_DIR/runner-wasm.mjs" "$WORKLOADS" | tee -a "$OUT_FILE"
      ;;
    go)
      note "go: go run"
      (cd "$BOUNDARY_DIR/runner-go" && go run . "$WORKLOADS") | tee -a "$OUT_FILE"
      ;;
    dotnet)
      note "dotnet: dotnet run -c Release"
      (cd "$REPO_ROOT/bindings/c" && cargo build --release)
      DATALOGIC_NATIVE_LIB="$C_LIB_DIR/libdatalogic_c.dylib" \
        dotnet run -c Release --project "$BOUNDARY_DIR/runner-dotnet" -- "$WORKLOADS" \
        | tee -a "$OUT_FILE"
      ;;
    jvm)
      note "jvm: mvn package + javac + java"
      (cd "$REPO_ROOT/bindings/jvm" && mvn -q -DskipTests package \
        && mvn -q -B dependency:build-classpath -Dmdep.outputFile=target/cp.txt)
      # Dependency classpath (Jackson) is needed at runtime: the binding's
      # batch item decoding uses ObjectMapper.
      JVM_CP="$REPO_ROOT/bindings/jvm/target/classes:$(cat "$REPO_ROOT/bindings/jvm/target/cp.txt")"
      javac -cp "$JVM_CP" \
        -d "$BUILD_DIR/jvm" "$BOUNDARY_DIR/runner-jvm/Boundary.java"
      java -cp "$JVM_CP:$BUILD_DIR/jvm" \
        --enable-native-access=ALL-UNNAMED \
        -Ddatalogic.library.path="$C_LIB_DIR" \
        Boundary "$WORKLOADS" | tee -a "$OUT_FILE"
      ;;
    php)
      note "php: run with JIT"
      (cd "$REPO_ROOT/bindings/c" && cargo build --release)
      DATALOGIC_NATIVE_LIB="$C_LIB_DIR/libdatalogic_c.dylib" \
        php -d opcache.enable_cli=1 -d opcache.jit=tracing -d opcache.jit_buffer_size=64M \
        "$BOUNDARY_DIR/runner-php.php" "$WORKLOADS" | tee -a "$OUT_FILE"
      ;;
    *)
      echo "run.sh: unknown runtime '$rt' (known: $DEFAULT_RUNTIMES $EXTENDED_RUNTIMES)" >&2
      exit 1
      ;;
  esac
done

printf '\nresults: %s\nrender:  python3 %s/render.py %s\n' \
  "$OUT_FILE" "$BOUNDARY_DIR" "$OUT_FILE" >&2
