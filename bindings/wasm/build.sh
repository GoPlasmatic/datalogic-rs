#!/bin/bash
set -e

# Build script for wasm npm package
# Builds for web, bundler, and nodejs targets and creates a unified package

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Version source: this crate's own Cargo.toml (the npm package version
# tracks the wasm crate, not the workspace root which has no `version`).
VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
echo "Building @goplasmatic/datalogic-wasm version $VERSION"

# Build-profile selection. The default is the size-optimized release
# profile (opt-level "z" + wasm-opt -Oz) — a plain `./build.sh` behaves
# exactly as it always has. `WASM_PROFILE=speed ./build.sh` opts into the
# speed profile ([profile.speed] in Cargo.toml: opt-level 3, paired with
# wasm-opt -O3): a larger module that evaluates faster. Measured
# size/speed tradeoff in the README's "Build profiles" section.
WASM_PROFILE="${WASM_PROFILE:-release}"
case "$WASM_PROFILE" in
  release)
    CARGO_PROFILE_FLAG="--release"
    WASM_OPT_LEVEL="-Oz"
    ;;
  speed)
    CARGO_PROFILE_FLAG="--profile speed"
    WASM_OPT_LEVEL="-O3"
    echo "  build profile: speed (opt-in; default is release / -Oz)"
    ;;
  *)
    echo "error: WASM_PROFILE must be 'release' (default) or 'speed', got '$WASM_PROFILE'" >&2
    exit 1
    ;;
esac

# Clean previous builds
rm -rf pkg pkg-web pkg-bundler pkg-nodejs

# Build for each target ($CARGO_PROFILE_FLAG is intentionally unquoted:
# the speed variant expands to two words, `--profile speed`).
echo "Building for web target..."
wasm-pack build --target web --out-dir pkg-web $CARGO_PROFILE_FLAG

echo "Building for bundler target..."
wasm-pack build --target bundler --out-dir pkg-bundler $CARGO_PROFILE_FLAG

echo "Building for nodejs target..."
wasm-pack build --target nodejs --out-dir pkg-nodejs $CARGO_PROFILE_FLAG

# Create unified package structure
echo "Creating unified package..."
mkdir -p pkg/web pkg/bundler pkg/nodejs

# Copy web target files
cp pkg-web/datalogic_wasm_bg.wasm pkg/web/
cp pkg-web/datalogic_wasm.js pkg/web/
cp pkg-web/datalogic_wasm.d.ts pkg/web/
cp pkg-web/datalogic_wasm_bg.wasm.d.ts pkg/web/

# Copy bundler target files
cp pkg-bundler/datalogic_wasm_bg.wasm pkg/bundler/
cp pkg-bundler/datalogic_wasm.js pkg/bundler/
cp pkg-bundler/datalogic_wasm.d.ts pkg/bundler/
cp pkg-bundler/datalogic_wasm_bg.wasm.d.ts pkg/bundler/
cp pkg-bundler/datalogic_wasm_bg.js pkg/bundler/

# Copy nodejs target files
cp pkg-nodejs/datalogic_wasm_bg.wasm pkg/nodejs/
cp pkg-nodejs/datalogic_wasm.js pkg/nodejs/
cp pkg-nodejs/datalogic_wasm.d.ts pkg/nodejs/
cp pkg-nodejs/datalogic_wasm_bg.wasm.d.ts pkg/nodejs/

# Per-subdir `package.json` overrides. The pkg root sets `"type": "module"`
# (so `web/` ESM resolves), but wasm-pack's `nodejs` and `bundler` targets
# emit CommonJS files using `exports.foo = ...`. Without these overrides,
# Node treats every .js in the package as ESM and the CJS files explode at
# import time with `ReferenceError: exports is not defined in ES module scope`.
echo '{"type":"commonjs"}' > pkg/nodejs/package.json
echo '{"type":"commonjs"}' > pkg/bundler/package.json

# Optimize WASM binaries with wasm-opt if available (-Oz for the default
# release profile, -O3 for the opt-in speed profile).
if command -v wasm-opt &> /dev/null; then
    echo "Optimizing WASM binaries with wasm-opt..."
    WASM_OPT_FLAGS="$WASM_OPT_LEVEL --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext"

    for target in web bundler nodejs; do
        WASM_FILE="pkg/$target/datalogic_wasm_bg.wasm"
        ORIGINAL_SIZE=$(stat -f%z "$WASM_FILE" 2>/dev/null || stat -c%s "$WASM_FILE")
        wasm-opt $WASM_OPT_FLAGS "$WASM_FILE" -o "$WASM_FILE.opt"
        mv "$WASM_FILE.opt" "$WASM_FILE"
        NEW_SIZE=$(stat -f%z "$WASM_FILE" 2>/dev/null || stat -c%s "$WASM_FILE")
        SAVED=$((ORIGINAL_SIZE - NEW_SIZE))
        echo "  $target: $ORIGINAL_SIZE -> $NEW_SIZE bytes (saved $SAVED bytes)"
    done
else
    echo "Warning: wasm-opt not found. Install binaryen for additional size optimization."
    echo "  brew install binaryen  # macOS"
    echo "  apt install binaryen   # Debian/Ubuntu"
fi

# Copy LICENSE (from this crate's root — the wasm crate ships its own
# copy alongside `Cargo.toml`) and README.
cp LICENSE pkg/
cp README.md pkg/

# Create package.json
cat > pkg/package.json << EOF
{
  "name": "@goplasmatic/datalogic-wasm",
  "version": "$VERSION",
  "description": "JSONLogic (json-logic) rules engine for browsers, edge, Deno, Bun, and Node — Rust core compiled to WebAssembly. A fast alternative to json-logic-js with identical semantics across 8 runtimes; flagd-compatible operators for OpenFeature-style feature flags.",
  "license": "Apache-2.0",
  "repository": {
    "type": "git",
    "url": "https://github.com/GoPlasmatic/datalogic-rs"
  },
  "homepage": "https://github.com/GoPlasmatic/datalogic-rs",
  "bugs": {
    "url": "https://github.com/GoPlasmatic/datalogic-rs/issues"
  },
  "keywords": [
    "jsonlogic",
    "json-logic",
    "json-logic-js",
    "rules-engine",
    "business-rules",
    "expression-engine",
    "feature-flags",
    "openfeature",
    "flagd",
    "wasm",
    "webassembly",
    "browser"
  ],
  "type": "module",
  "main": "./nodejs/datalogic_wasm.js",
  "module": "./web/datalogic_wasm.js",
  "types": "./web/datalogic_wasm.d.ts",
  "exports": {
    ".": {
      "node": {
        "types": "./nodejs/datalogic_wasm.d.ts",
        "default": "./nodejs/datalogic_wasm.js"
      },
      "import": {
        "types": "./web/datalogic_wasm.d.ts",
        "default": "./web/datalogic_wasm.js"
      },
      "require": {
        "types": "./bundler/datalogic_wasm.d.ts",
        "default": "./bundler/datalogic_wasm.js"
      },
      "default": {
        "types": "./web/datalogic_wasm.d.ts",
        "default": "./web/datalogic_wasm.js"
      }
    },
    "./web": {
      "types": "./web/datalogic_wasm.d.ts",
      "default": "./web/datalogic_wasm.js"
    },
    "./bundler": {
      "types": "./bundler/datalogic_wasm.d.ts",
      "default": "./bundler/datalogic_wasm.js"
    },
    "./nodejs": {
      "types": "./nodejs/datalogic_wasm.d.ts",
      "default": "./nodejs/datalogic_wasm.js"
    }
  },
  "files": [
    "web/",
    "bundler/",
    "nodejs/",
    "LICENSE",
    "README.md"
  ],
  "engines": {
    "node": ">=16.0.0"
  },
  "sideEffects": false
}
EOF

# Clean up temporary directories
rm -rf pkg-web pkg-bundler pkg-nodejs

echo ""
echo "Build complete! Package created in pkg/"
echo ""
echo "To publish:"
echo "  cd pkg"
echo "  npm login --scope=@goplasmatic"
echo "  npm publish --access public"
echo ""
echo "To test locally:"
echo "  cd pkg && npm pack"
echo "  # In your test project:"
echo "  npm install /path/to/goplasmatic-datalogic-wasm-$VERSION.tgz"
