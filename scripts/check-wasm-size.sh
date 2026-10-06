#!/usr/bin/env bash
# WASM bundle-size gate.
#
# Compares the gzip size of the shipped engine module
# (pkg/web/datalogic_wasm_bg.wasm; the bundler and nodejs targets carry
# the same bytes) against the committed baseline in
# scripts/wasm-size.baseline. Above +3% it warns; above +5% it fails, the
# growth budget proposal-v6 §7 S3 allows a change before it has to argue
# for its size.
#
# Usage:
#   scripts/check-wasm-size.sh [PKG_DIR]            check (default bindings/wasm/pkg)
#   scripts/check-wasm-size.sh --update [PKG_DIR]   rewrite the baseline
#
# Update the baseline in the same commit as a deliberate size change (or a
# binaryen bump, see .github/actions/install-binaryen), built with
# `cd bindings/wasm && ./build.sh` and the pinned binaryen on PATH.
set -euo pipefail

cd "$(dirname "$0")/.."

update=0
if [ "${1:-}" = "--update" ]; then update=1; shift; fi
pkg=${1:-bindings/wasm/pkg}
baseline_file=scripts/wasm-size.baseline
wasm="$pkg/web/datalogic_wasm_bg.wasm"

[ -f "$wasm" ] || { echo "no $wasm; build it with bindings/wasm/build.sh" >&2; exit 2; }

# -n: no name or timestamp in the header, so the size depends on the bytes
# alone.
size=$(gzip -9 -n -c "$wasm" | wc -c | tr -d ' ')
raw=$(wc -c <"$wasm" | tr -d ' ')

if [ "$update" -eq 1 ]; then
  {
    echo "# gzip -9 -n size in bytes of bindings/wasm/pkg/web/datalogic_wasm_bg.wasm."
    echo "# Written by scripts/check-wasm-size.sh --update; see that script."
    echo "$size"
  } >"$baseline_file"
  echo "wasm size baseline: $size bytes gzip ($raw raw)"
  exit 0
fi

baseline=$(grep -v '^#' "$baseline_file" | tr -d ' \n')
pct=$(awk -v s="$size" -v b="$baseline" 'BEGIN { printf "%+.2f", (s - b) * 100 / b }')
echo "wasm size: $size bytes gzip ($raw raw); baseline $baseline ($pct%)"

notice() {  # level, message
  echo "$1: $2" >&2
  [ -z "${GITHUB_ACTIONS:-}" ] || echo "::$1::$2"
}
if [ "$size" -gt $((baseline * 105 / 100)) ]; then
  notice error "WASM module grew $pct% over the baseline (limit +5%). If the growth is intended, run scripts/check-wasm-size.sh --update and commit the new baseline with the change."
  exit 1
elif [ "$size" -gt $((baseline * 103 / 100)) ]; then
  notice warning "WASM module grew $pct% over the baseline (warning above +3%, failure above +5%)."
elif [ "$size" -lt $((baseline * 95 / 100)) ]; then
  notice notice "WASM module shrank $pct%; consider lowering the baseline with scripts/check-wasm-size.sh --update."
fi
