#!/bin/bash
set -e

# Publish script for @goplasmatic/datalogic-ui package
# Builds and publishes to npm

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Get version from package.json
VERSION=$(grep '"version"' package.json | head -1 | sed 's/.*"version": "\(.*\)".*/\1/')
echo "Building @goplasmatic/datalogic-ui@$VERSION..."

# Build the library
pnpm build:lib

echo ""
echo "Publishing @goplasmatic/datalogic-ui@$VERSION to npm..."

# Publish (use pnpm to resolve workspace:* protocol to actual version)
pnpm publish --access public --no-git-checks

echo ""
echo "Published @goplasmatic/datalogic-ui@$VERSION successfully!"
