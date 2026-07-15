#!/usr/bin/env bash
# CI guard against marketing-stat drift in living documents.
#
# The repo quotes a small set of headline numbers (conformance suite/case
# counts, benchmark geomean) in READMEs, badges, and docs. This script
# fails when a quoted stat no longer matches its canonical source, or
# when a known-stale figure reappears.
#
# Canonical sources:
#   - scripts/conformance-count.sh  → "<N> suites / <M> cases"
#   - tools/benchmark/BENCHMARK.md  → performance geomeans (the GEOMEAN
#     constant below; update it when the quarterly benchmark refresh
#     lands, in the same commit that updates BENCHMARK.md)
#
# CHANGELOG.md is exempt everywhere: it is a historical record.
set -euo pipefail

cd "$(dirname "$0")/.."

fail=0
err() { echo "FAIL: $*" >&2; fail=1; }

# --- canonical values ---------------------------------------------------
stat=$(bash scripts/conformance-count.sh)   # e.g. "53 suites / 1,532 cases"
suites=${stat%% suites*}
cases=${stat##*/ }
cases=${cases%% cases*}
GEOMEAN="8.9 ns"   # BENCHMARK.md cross-library geomean, captured 2026-07-03

# --- canonical strings must appear where we quote them ------------------
badge="conformance-${suites}_suites_%2F_${cases}_cases"
grep -qF "$badge" README.md \
  || err "README.md conformance badge does not encode '$stat' (expected '$badge')"
grep -qF "${cases}-case" README.md \
  || err "README.md prose does not quote the ${cases}-case battery"
grep -qF "${suites}-suite" README.md \
  || err "README.md prose does not quote the ${suites}-suite battery"
grep -qF "$GEOMEAN" README.md \
  || err "README.md does not quote the canonical $GEOMEAN geomean"
grep -qF "$GEOMEAN" crates/datalogic-rs/README.md \
  || err "crates/datalogic-rs/README.md does not quote the canonical $GEOMEAN geomean"

# --- known-stale figures must not reappear in living documents ----------
# Each pattern is a number we have already had to scrub once. Extend this
# list whenever a refresh retires a previously-quoted figure.
stale_patterns=(
  '9\.7 ns'
  '44 operator suites'
  'Maven release pending'
)
for pat in "${stale_patterns[@]}"; do
  hits=$(grep -rEln "$pat" \
    --include='*.md' \
    --exclude='CHANGELOG.md' \
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
  echo "Stats drifted. Canonical sources: scripts/conformance-count.sh" >&2
  echo "and tools/benchmark/BENCHMARK.md. Fix the documents (or, after a" >&2
  echo "benchmark refresh, the constants in this script)." >&2
  exit 1
fi

echo "check-stats: OK ($stat, geomean $GEOMEAN)"
