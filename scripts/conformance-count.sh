#!/usr/bin/env bash
# Prints the canonical conformance-suite statistic quoted in READMEs,
# docs, badges, and release notes: "<N> suites / <M> cases".
#
# Counts only suites listed in tests/suites/index.json (what the
# conformance runner actually executes) and only dict entries within
# them (strings are section headers, skipped by the runner).
#
# Regenerate before every release and update any hardcoded quote that
# drifted:   ./scripts/conformance-count.sh
set -euo pipefail

cd "$(dirname "$0")/../crates/datalogic-rs/tests/suites"

python3 - <<'EOF'
import json

index = json.load(open("index.json"))
total = 0
for suite in index:
    cases = json.load(open(suite))
    total += sum(1 for c in cases if isinstance(c, dict))
print(f"{len(index)} suites / {total:,} cases")
EOF
