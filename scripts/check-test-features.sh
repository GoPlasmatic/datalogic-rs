#!/usr/bin/env bash
# Keep the core crate's `[[test]] required-features` in step with the
# `#![cfg(feature = ...)]` header of each integration test.
#
# A test file gated on a feature compiles to an empty binary when the
# feature is off, and that binary reports "0 passed" as success. Declaring
# the same features as `required-features` makes cargo skip the target
# instead, and makes `cargo test --test <name>` without them an error that
# names the missing features. This check fails when a gated file has no
# entry, when an entry's features differ from the file's header, or when an
# entry names a file that no longer exists.
set -euo pipefail

cd "$(dirname "$0")/../crates/datalogic-rs"

python3 - <<'EOF'
import glob
import os
import re
import sys
import tomllib

with open("Cargo.toml", "rb") as f:
    manifest = tomllib.load(f)
declared = {t["name"]: set(t.get("required-features", [])) for t in manifest.get("test", [])}

problems = []
gated = {}
for path in sorted(glob.glob("tests/*.rs")):
    name = os.path.basename(path)[:-3]
    text = open(path, encoding="utf-8").read()
    m = re.search(r"^#!\[cfg\((.*?)\)\]\s*$", text, re.S | re.M)
    if not m:
        continue
    cfg = " ".join(m.group(1).split())
    feats = re.findall(r'feature\s*=\s*"([^"]+)"', cfg)
    if not feats:
        continue
    if re.search(r"\b(any|not)\s*\(", cfg):
        problems.append(f"{path}: cfg header `{cfg}` is not a plain conjunction; "
                        "required-features can only express all(...)")
        continue
    gated[name] = set(feats)

for name, feats in gated.items():
    if name not in declared:
        problems.append(f"tests/{name}.rs is gated on {sorted(feats)} but Cargo.toml has no "
                        f"[[test]] entry for it")
    elif declared[name] != feats:
        problems.append(f"tests/{name}.rs is gated on {sorted(feats)} but its [[test]] entry "
                        f"requires {sorted(declared[name])}")
for name in declared:
    if not os.path.exists(f"tests/{name}.rs") and not os.path.exists(f"tests/{name}/main.rs"):
        problems.append(f"[[test]] entry `{name}` names no file under tests/")

for p in problems:
    print(f"FAIL: {p}", file=sys.stderr)
if problems:
    print("\nAdd or fix the `[[test]]` entries at the end of crates/datalogic-rs/Cargo.toml.",
          file=sys.stderr)
    sys.exit(1)
print(f"check-test-features: OK ({len(gated)} gated integration tests)")
EOF
