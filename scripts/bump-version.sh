#!/usr/bin/env bash
# Bump the release version across every file whose version field the
# release workflow checks for consistency.
#
# SOURCE OF TRUTH: .github/workflows/release.yml, `validate` job (the
# "Extract version" and "Validate binding versions match core" steps).
# The FILES list below mirrors that job exactly; if a versioned package
# is added there, add it here too. Every file is pattern-checked before
# and after the edit, so drift between this script and the real files
# fails loudly here instead of at tag time.
#
# Deliberately NOT bumped (mirrors release.yml):
#   - bindings/php/composer.json: carries no version field; Packagist
#     resolves versions from the pushed v* tag.
#   - Go module: version lives in the `bindings/go/vX.Y.Z` tag pushed
#     by publish-go; nothing in source to bump.
#   - WASM npm package: wasm-pack generates pkg/package.json from
#     bindings/wasm/Cargo.toml at build time, so bumping that
#     Cargo.toml covers it.
#
# Edits use `perl -pi -e` because `sed -i` is not portable between the
# GNU (Linux) and BSD (macOS) sed dialects.
#
# Usage: scripts/bump-version.sh <x.y.z>
set -euo pipefail

NEW=${1:?usage: bump-version.sh <x.y.z>}
if ! [[ "$NEW" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "error: '$NEW' is not a plain x.y.z semver version" >&2
  exit 2
fi
export NEW

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

# kind:path pairs. `kind` picks the version pattern; every replacement
# touches only the FIRST match in the file, matching the `head -1`
# semantics of the extractors in release.yml (pom.xml has dozens of
# dependency/plugin <version> tags after the project one, and
# bindings/node/package.json has a `"version": "napi version"` script
# entry further down).
FILES=(
  "toml:crates/datalogic-rs/Cargo.toml"
  "toml:bindings/python/Cargo.toml"
  "toml:bindings/wasm/Cargo.toml"
  "toml:bindings/c/Cargo.toml"
  "toml:bindings/node/Cargo.toml"
  "toml:bindings/python/pyproject.toml"
  "json:bindings/node/package.json"
  "json:ui/package.json"
  "csproj:bindings/dotnet/src/Datalogic/Datalogic.csproj"
  "pom:bindings/jvm/pom.xml"
)

# Extract the current version the same way release.yml's validate job
# does. Its `grep -oP` / `node -p` extractors are rewritten with
# portable sed so the script also runs on stock macOS (BSD grep has no
# -P) and without node.
extract() {
  local kind=$1 file=$2
  case "$kind" in
    toml)   grep '^version = ' "$file" | head -1 | sed 's/version = "\(.*\)"/\1/' ;;
    json)   sed -n 's/^[[:space:]]*"version": *"\([^"]*\)".*/\1/p' "$file" | head -1 ;;
    csproj) sed -n 's/.*<Version>\([^<]*\)<\/Version>.*/\1/p' "$file" | head -1 ;;
    pom)    sed -n 's/.*<version>\([^<]*\)<\/version>.*/\1/p' "$file" | head -1 ;;
  esac
}

# Replace only the first match in the file ($done latches after the
# first successful substitution).
replace_first() {
  local kind=$1 file=$2
  case "$kind" in
    toml)   perl -pi -e '$done ||= s/^version = "[^"]*"/version = "$ENV{NEW}"/' "$file" ;;
    json)   perl -pi -e '$done ||= s/("version":\s*")[^"]*(")/${1}$ENV{NEW}${2}/' "$file" ;;
    csproj) perl -pi -e '$done ||= s{<Version>[^<]*<\/Version>}{<Version>$ENV{NEW}<\/Version>}' "$file" ;;
    pom)    perl -pi -e '$done ||= s{<version>[^<]*<\/version>}{<version>$ENV{NEW}<\/version>}' "$file" ;;
  esac
}

echo "Bumping to $NEW:"
for entry in "${FILES[@]}"; do
  kind=${entry%%:*}
  file=${entry#*:}
  if [ ! -f "$file" ]; then
    echo "error: $file not found. Did the file set drift from release.yml's validate job?" >&2
    exit 1
  fi
  before=$(extract "$kind" "$file" || true)
  if [ -z "$before" ]; then
    echo "error: no version pattern found in $file. Did the file set drift from release.yml's validate job?" >&2
    exit 1
  fi
  replace_first "$kind" "$file"
  after=$(extract "$kind" "$file" || true)
  if [ "$after" != "$NEW" ]; then
    echo "error: replacement failed for $file (found '$after', expected '$NEW')" >&2
    exit 1
  fi
  printf '  %-48s %s -> %s\n' "$file" "$before" "$after"
done

# Re-run the same consistency check release.yml's validate job performs
# at tag time: every binding version must equal the core crate's.
echo
echo "Consistency check (mirrors release.yml validate job):"
CORE=$(extract toml crates/datalogic-rs/Cargo.toml || true)
FAIL=0
for entry in "${FILES[@]}"; do
  kind=${entry%%:*}
  file=${entry#*:}
  v=$(extract "$kind" "$file" || true)
  if [ "$v" != "$CORE" ]; then
    echo "  $file: version is '$v', expected '$CORE' (bump together with core)"
    FAIL=1
  else
    echo "  $file: $v ✓"
  fi
done
if [ "$FAIL" != 0 ]; then
  echo "error: one or more binding versions drift from core ($CORE)" >&2
  exit 1
fi
echo "All binding versions match core $CORE"

# Not fatal here, but release.yml's validate job also requires a dated
# '## [X.Y.Z] - YYYY-MM-DD' CHANGELOG section before tagging.
if ! grep -Eq "^## \[${NEW}\] - [0-9]{4}-[0-9]{2}-[0-9]{2}[[:space:]]*$" CHANGELOG.md 2>/dev/null; then
  echo
  echo "note: CHANGELOG.md has no '## [${NEW}] - YYYY-MM-DD' entry yet;"
  echo "      release.yml's validate job will fail at tag time without one."
fi
