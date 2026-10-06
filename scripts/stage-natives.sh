#!/usr/bin/env bash
# Stage the per-platform C ABI release artifacts into one binding's
# package layout. The single place that maps the release matrix's
# platforms onto each host's naming scheme.
#
# Usage: scripts/stage-natives.sh <scheme> <artifact-dir> [dest-dir]
#
#   scheme        jvm | dotnet | php | go
#   artifact-dir  holds one folder per platform, as downloaded (merged)
#                 from the release artifacts: `<os>-<arch>/` for the
#                 cdylib schemes, `<os>_<arch>/` for go (the staticlib
#                 artifacts root at Go's own GOOS_GOARCH naming)
#   dest-dir      defaults to the binding's in-tree location below
#
# Every platform the release matrix builds must be present: a missing one
# fails the run, instead of shipping a package that cannot load on that
# platform (the old per-workflow mappers skipped it with `if [ -d ]`).
#
# Release platforms and each host's name for them:
#
#   matrix         jvm (JNA prefix)  dotnet (RID)   php               go
#   linux-amd64    linux-x86-64      linux-x64      linux-x86_64      linux_amd64
#   linux-arm64    linux-aarch64     linux-arm64    linux-aarch64     linux_arm64
#   darwin-amd64   darwin-x86-64     osx-x64        darwin-x86_64     darwin_amd64
#   darwin-arm64   darwin-aarch64    osx-arm64      darwin-aarch64    darwin_arm64
#   windows-amd64  win32-x86-64      win-x64        windows-x86_64    windows_amd64
#   windows-arm64  win32-aarch64     win-arm64      windows-aarch64   windows_arm64
#
# The loaders that read these names: jvm internal/NativeLibrary.java,
# dotnet NuGet's runtimes/<rid>/native convention, php
# src/Internal/Native.php (platformDir), go cgo_<os>_<arch>.go.
#
# Plain `case` rather than `declare -A`, so it also runs on macOS's stock
# bash 3.2.
set -euo pipefail

usage() { echo "usage: $0 <jvm|dotnet|php|go> <artifact-dir> [dest-dir]" >&2; exit 2; }
scheme=${1:-}
src=${2:-}
[ -n "$scheme" ] && [ -n "$src" ] || usage

case $scheme in
  jvm)    dest=${3:-bindings/jvm/src/main/resources} ;;
  dotnet) dest=${3:-bindings/dotnet/runtimes} ;;
  php)    dest=${3:-bindings/php/lib} ;;
  go)     dest=${3:-bindings/go/lib} ;;
  *)      usage ;;
esac

# The target directory, under $dest, for one matrix platform.
target_dir() {
  case "$scheme:$1" in
    jvm:linux-amd64)      echo linux-x86-64 ;;
    jvm:linux-arm64)      echo linux-aarch64 ;;
    jvm:darwin-amd64)     echo darwin-x86-64 ;;
    jvm:darwin-arm64)     echo darwin-aarch64 ;;
    jvm:windows-amd64)    echo win32-x86-64 ;;
    jvm:windows-arm64)    echo win32-aarch64 ;;
    dotnet:linux-amd64)   echo linux-x64/native ;;
    dotnet:linux-arm64)   echo linux-arm64/native ;;
    dotnet:darwin-amd64)  echo osx-x64/native ;;
    dotnet:darwin-arm64)  echo osx-arm64/native ;;
    dotnet:windows-amd64) echo win-x64/native ;;
    dotnet:windows-arm64) echo win-arm64/native ;;
    php:linux-amd64)      echo linux-x86_64 ;;
    php:linux-arm64)      echo linux-aarch64 ;;
    php:darwin-amd64)     echo darwin-x86_64 ;;
    php:darwin-arm64)     echo darwin-aarch64 ;;
    php:windows-amd64)    echo windows-x86_64 ;;
    php:windows-arm64)    echo windows-aarch64 ;;
    go:*)                 echo "${1/-/_}" ;;
  esac
}

missing=0
for plat in linux-amd64 linux-arm64 darwin-amd64 darwin-arm64 windows-amd64 windows-arm64; do
  if [ "$scheme" = go ]; then from="$src/${plat/-/_}"; else from="$src/$plat"; fi
  if [ ! -d "$from" ] || [ -z "$(ls -A "$from")" ]; then
    echo "::error::stage-natives ($scheme): no artifact for $plat (looked in $from)" >&2
    missing=1
    continue
  fi
  to="$dest/$(target_dir "$plat")"
  mkdir -p "$to"
  cp "$from"/* "$to/"
  echo "Staged $plat -> $to"
done

if [ "$missing" -ne 0 ]; then
  echo "stage-natives: refusing to package $scheme with platforms missing" >&2
  exit 1
fi
ls -R "$dest"
