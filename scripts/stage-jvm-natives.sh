#!/usr/bin/env bash
# Stage cdylib release artifacts at the JVM binding's classpath root.
#
# The binding's loader (internal/NativeLibrary) extracts a bundled
# native from `<os-arch>/<libname>` at the classpath ROOT (e.g.
# `darwin-aarch64/libdatalogic_c.dylib`), NOT from `META-INF/native/`.
# The <os-arch> strings deliberately keep JNA's historical
# Platform.RESOURCE_PREFIX naming — the layout survived the JNA→FFM
# rewrite unchanged. Both the release build job (release-build-jvm.yml)
# and the Maven Central publish job (release.yml) stage through this
# script so the two layouts cannot drift apart.
#
# Usage: scripts/stage-jvm-natives.sh <cdylib-artifact-dir>
#   <cdylib-artifact-dir> contains one <os>-<arch>/ folder per platform,
#   as downloaded from the c-cdylib-* release artifacts.
set -euo pipefail

src=${1:?usage: stage-jvm-natives.sh <cdylib-artifact-dir>}
dest=bindings/jvm/src/main/resources

# case-based mapping instead of `declare -A` so the script also runs on
# macOS's stock bash 3.2 (CI runners have bash 5, local devs may not)
for plat in linux-amd64 linux-arm64 darwin-amd64 darwin-arm64 windows-amd64 windows-arm64; do
  case $plat in
    linux-amd64) prefix=linux-x86-64 ;;
    linux-arm64) prefix=linux-aarch64 ;;
    darwin-amd64) prefix=darwin-x86-64 ;;
    darwin-arm64) prefix=darwin-aarch64 ;;
    windows-amd64) prefix=win32-x86-64 ;;
    windows-arm64) prefix=win32-aarch64 ;;
  esac
  if [ -d "$src/$plat" ]; then
    mkdir -p "$dest/$prefix"
    cp "$src/$plat/"* "$dest/$prefix/"
    echo "Staged $plat -> $prefix"
  fi
done
ls -R "$dest"
