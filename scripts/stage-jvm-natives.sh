#!/usr/bin/env bash
# Stage cdylib release artifacts at the JVM binding's classpath root.
#
# JNA's `Native.load` extracts a bundled native from
# `<Platform.RESOURCE_PREFIX>/<libname>` at the classpath ROOT (e.g.
# `darwin-aarch64/libdatalogic_c.dylib`), NOT from `META-INF/native/`.
# Both the release build job (release-build-jvm.yml) and the Maven
# Central publish job (release.yml) stage through this script so the
# two layouts cannot drift apart.
#
# Usage: scripts/stage-jvm-natives.sh <cdylib-artifact-dir>
#   <cdylib-artifact-dir> contains one <os>-<arch>/ folder per platform,
#   as downloaded from the c-cdylib-* release artifacts.
set -euo pipefail

src=${1:?usage: stage-jvm-natives.sh <cdylib-artifact-dir>}
dest=bindings/jvm/src/main/resources

declare -A JNA=(
  [linux-amd64]=linux-x86-64
  [linux-arm64]=linux-aarch64
  [darwin-amd64]=darwin-x86-64
  [darwin-arm64]=darwin-aarch64
  [windows-amd64]=win32-x86-64
  [windows-arm64]=win32-aarch64
)

for plat in "${!JNA[@]}"; do
  jna=${JNA[$plat]}
  if [ -d "$src/$plat" ]; then
    mkdir -p "$dest/$jna"
    cp "$src/$plat/"* "$dest/$jna/"
    echo "Staged $plat -> $jna"
  fi
done
ls -R "$dest"
