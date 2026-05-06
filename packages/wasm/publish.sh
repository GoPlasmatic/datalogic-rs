#!/bin/bash
set -e

# Publish script for @goplasmatic/datalogic WASM package
# Builds and publishes to npm

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Run the build script
echo "Building WASM package..."
./build.sh

# Get version from package.json
VERSION=$(grep '"version"' pkg/package.json | sed 's/.*"version": "\(.*\)".*/\1/')
echo ""
echo "Publishing @goplasmatic/datalogic@$VERSION to npm..."

# Publish
cd pkg
npm publish --access public

echo ""
echo "Published @goplasmatic/datalogic@$VERSION successfully!"
