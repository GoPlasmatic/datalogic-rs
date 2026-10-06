#!/usr/bin/env bash
# CI guard against stat drift in living documents.
#
# The repo quotes a small set of headline numbers (conformance suite/case
# counts, benchmark geomean, the MSRV) in READMEs, badges, and docs. This
# script checks each quote against its canonical source, and fails when a
# known-stale figure reappears.
#
# Canonical sources:
#   - scripts/conformance-count.sh  → "<N> suites / <M> cases"
#   - tools/benchmark/BENCHMARK.md  → performance geomeans (the GEOMEAN
#     constant below; update it when the quarterly benchmark refresh
#     lands, in the same commit that updates BENCHMARK.md)
#   - crates/datalogic-rs/Cargo.toml `rust-version` → the MSRV quoted in
#     CONTRIBUTING.md, DEVELOPMENT.md, the crate README badge and the
#     installation page
#
# Usage:
#   scripts/check-stats.sh            PR mode: conformance-count drift is a
#                                     warning, everything else fails
#   scripts/check-stats.sh --strict   release mode: count drift fails too
#   scripts/check-stats.sh --write    rewrite every conformance-count quote
#                                     to the canonical numbers, then check
#
# Why counts only warn on PRs: every new suite case changes the canonical
# count, and failing the PR for it tied every test addition to a README
# edit (and turned the next unrelated PR red when a docs-only change
# skipped CI). The release `validate` job runs `--strict`, so a release
# can never ship stale numbers; `--write` is the one-command fix.
#
# CHANGELOG.md and TECH_DEBT.md are exempt everywhere: they are historical
# records.
set -euo pipefail

cd "$(dirname "$0")/.."

mode=check
case "${1:-}" in
  "") ;;
  --strict) mode=strict ;;
  --write) mode=write ;;
  -h|--help) sed -n '2,31p' "$0"; exit 0 ;;
  *) echo "usage: $0 [--strict|--write]" >&2; exit 2 ;;
esac

fail=0
err() { echo "FAIL: $*" >&2; fail=1; }

# --- canonical values ---------------------------------------------------
stat=$(bash scripts/conformance-count.sh)   # e.g. "53 suites / 1,532 cases"
suites=${stat%% suites*}
cases=${stat##*/ }
cases=${cases%% cases*}
GEOMEAN="10.3 ns"   # BENCHMARK.md cross-library geomean, captured 2026-07-17
msrv=$(sed -n 's/^rust-version = "\(.*\)"/\1/p' crates/datalogic-rs/Cargo.toml | head -1)
[ -n "$msrv" ] || err "crates/datalogic-rs/Cargo.toml declares no rust-version"

# --- conformance counts -------------------------------------------------
# Every phrasing the docs use to quote the battery. The sweep covers every
# living Markdown/text file, so a quote in a binding README drifts no more
# quietly than the root README's badge. `--write` rewrites them in place.
count_out=$(SUITES="$suites" CASES="$cases" MODE="$mode" python3 - <<'EOF'
import os, re, sys

suites, cases, mode = os.environ["SUITES"], os.environ["CASES"], os.environ["MODE"]
patterns = [
    # (regex, replacement) — the replacement carries the canonical numbers.
    (r"conformance-\d+_suites_%2F_[\d,]+_cases", f"conformance-{suites}_suites_%2F_{cases}_cases"),
    (r"\b[\d,]+-case conformance", f"{cases}-case conformance"),
    (r"\b\d+-suite conformance", f"{suites}-suite conformance"),
    (r"\b[\d,]+ cases across \d+ suites", f"{cases} cases across {suites} suites"),
    (r"(conformance\s+(?:battery|suite)\s+)\(\d+ suites\)", rf"\g<1>({suites} suites)"),
]
skip_dirs = {"node_modules", "target", "dist", "dist-embed", "book", ".git", "vendor", ".venv"}
skip_files = {"CHANGELOG.md", "TECH_DEBT.md"}
drift = []
for root, dirs, files in os.walk("."):
    dirs[:] = sorted(d for d in dirs if d not in skip_dirs)
    for name in sorted(files):
        if not name.endswith((".md", ".txt")) or name in skip_files:
            continue
        path = os.path.join(root, name)[2:]
        with open(path, encoding="utf-8") as f:
            text = f.read()
        new = text
        for regex, repl in patterns:
            for m in re.finditer(regex, new):
                if m.group(0) != re.sub(regex, repl, m.group(0)):
                    line = new.count("\n", 0, m.start()) + 1
                    quote = " ".join(m.group(0).split())
                    drift.append(f"{path}:{line}: '{quote}'")
            new = re.sub(regex, repl, new)
        if mode == "write" and new != text:
            with open(path, "w", encoding="utf-8") as f:
                f.write(new)
            print(f"WROTE {path}")
if mode != "write":
    for d in drift:
        print(f"DRIFT {d}")
EOF
)
if [ -n "$count_out" ]; then
  while IFS= read -r line; do
    case $line in
      WROTE*) echo "$line" ;;
      DRIFT*)
        msg="conformance count quote does not match '$stat': ${line#DRIFT }"
        if [ "$mode" = strict ]; then
          err "$msg"
        else
          echo "WARN: $msg" >&2
          # Surface it on the PR without failing the job.
          [ -z "${GITHUB_ACTIONS:-}" ] || echo "::warning::$msg"
          count_drift=1
        fi
        ;;
    esac
  done <<<"$count_out"
fi

# The badge must exist at all (the sweep above only checks quotes that are
# there).
grep -qE "conformance-[0-9]+_suites_%2F_[0-9,]+_cases" README.md \
  || err "README.md has no conformance badge"

# --- geomean ------------------------------------------------------------
grep -qF "$GEOMEAN" README.md \
  || err "README.md does not quote the canonical $GEOMEAN geomean"
grep -qF "$GEOMEAN" crates/datalogic-rs/README.md \
  || err "crates/datalogic-rs/README.md does not quote the canonical $GEOMEAN geomean"

# --- MSRV ---------------------------------------------------------------
# Each file must quote the floor Cargo.toml declares, in the phrasing it
# uses today.
if [ -n "$msrv" ]; then
  msrv_re=${msrv//./\\.}
  grep -qE "^\| Rust +\| ${msrv_re}\+ " DEVELOPMENT.md \
    || err "DEVELOPMENT.md prerequisites table does not quote Rust ${msrv}+"
  grep -qE "\*\*Rust\*\* ${msrv_re} or newer" CONTRIBUTING.md \
    || err "CONTRIBUTING.md does not quote Rust ${msrv} or newer"
  grep -qF "badge/rust-${msrv}+-" crates/datalogic-rs/README.md \
    || err "crates/datalogic-rs/README.md MSRV badge does not say ${msrv}+"
  grep -qE "requires Rust \*\*${msrv_re}\*\*" docs/src/getting-started/installation.md \
    || err "docs/src/getting-started/installation.md does not quote Rust ${msrv}"
  grep -qE "dtolnay/rust-toolchain@${msrv_re}(\.0)?\b" .github/workflows/ci.yml \
    || err ".github/workflows/ci.yml msrv job does not install Rust ${msrv}"
fi

# --- known-stale figures must not reappear in living documents ----------
# Each pattern is a number we have already had to scrub once. Extend this
# list whenever a refresh retires a previously-quoted figure.
# docs/src/llms.txt is swept too: it is served to LLM crawlers and quoted
# the retired 8.9 ns / 1,565-case / 59-operator figures unnoticed.
stale_patterns=(
  '9\.7 ns'
  '8\.9 ns'
  '44 operator suites'
  '1,565'
  '59 built-in operators'
  '1,714'
  '59 suites'
  '64 built-in operators'
  'Maven release pending'
)
for pat in "${stale_patterns[@]}"; do
  hits=$(grep -rEln "$pat" \
    --include='*.md' \
    --include='*.txt' \
    --exclude='CHANGELOG.md' \
    --exclude='TECH_DEBT.md' \
    --exclude-dir=node_modules \
    --exclude-dir=target \
    --exclude-dir=dist \
    --exclude-dir=dist-embed \
    --exclude-dir=book \
    --exclude-dir=.git \
    . || true)
  [ -z "$hits" ] || err "stale stat '$pat' found in: $(echo "$hits" | tr '\n' ' ')"
done

if [ "$fail" -ne 0 ]; then
  echo >&2
  echo "Stats drifted. Canonical sources: scripts/conformance-count.sh," >&2
  echo "tools/benchmark/BENCHMARK.md and crates/datalogic-rs/Cargo.toml." >&2
  echo "Fix count quotes with 'scripts/check-stats.sh --write'; fix the" >&2
  echo "rest by hand (or, after a benchmark refresh, the constants here)." >&2
  exit 1
fi

if [ -n "${count_drift:-}" ]; then
  echo "check-stats: OK with count warnings ($stat, geomean $GEOMEAN, MSRV $msrv)"
  echo "  run 'scripts/check-stats.sh --write' to update the quotes"
else
  echo "check-stats: OK ($stat, geomean $GEOMEAN, MSRV $msrv)"
fi
