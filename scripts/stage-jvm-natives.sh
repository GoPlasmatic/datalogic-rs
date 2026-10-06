#!/usr/bin/env bash
# Stage cdylib release artifacts at the JVM binding's classpath root.
#
# The binding's loader (internal/NativeLibrary) extracts a bundled
# native from `<os-arch>/<libname>` at the classpath ROOT (e.g.
# `darwin-aarch64/libdatalogic_c.dylib`), NOT from `META-INF/native/`.
# The <os-arch> strings deliberately keep JNA's historical
# Platform.RESOURCE_PREFIX naming — the layout survived the JNA→FFM
# rewrite unchanged.
#
# The mapping itself lives in scripts/stage-natives.sh, shared with the
# .NET, PHP and Go packaging, and fails when a platform is missing.
#
# Usage: scripts/stage-jvm-natives.sh <cdylib-artifact-dir>
#   <cdylib-artifact-dir> contains one <os>-<arch>/ folder per platform,
#   as downloaded from the c-cdylib-* release artifacts.
set -euo pipefail

src=${1:?usage: stage-jvm-natives.sh <cdylib-artifact-dir>}
exec bash "$(dirname "$0")/stage-natives.sh" jvm "$src"
