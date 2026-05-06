#!/bin/bash
set -e

# Build script for wasm npm package
# Builds for web, bundler, and nodejs targets and creates a unified package

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Get version from root Cargo.toml
VERSION=$(grep '^version = ' ../Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
echo "Building @goplasmatic/datalogic version $VERSION"

# Clean previous builds
rm -rf pkg pkg-web pkg-bundler pkg-nodejs

# Build for each target
echo "Building for web target..."
wasm-pack build --target web --out-dir pkg-web --release

echo "Building for bundler target..."
wasm-pack build --target bundler --out-dir pkg-bundler --release

echo "Building for nodejs target..."
wasm-pack build --target nodejs --out-dir pkg-nodejs --release

# Create unified package structure
echo "Creating unified package..."
mkdir -p pkg/web pkg/bundler pkg/nodejs

# Copy web target files
cp pkg-web/datalogic_wasm_bg.wasm pkg/web/
cp pkg-web/datalogic_wasm.js pkg/web/
cp pkg-web/datalogic_wasm.d.ts pkg/web/
cp pkg-web/datalogic_wasm_bg.wasm.d.ts pkg/web/

# Copy bundler target files
cp pkg-bundler/datalogic_wasm_bg.wasm pkg/bundler/
cp pkg-bundler/datalogic_wasm.js pkg/bundler/
cp pkg-bundler/datalogic_wasm.d.ts pkg/bundler/
cp pkg-bundler/datalogic_wasm_bg.wasm.d.ts pkg/bundler/
cp pkg-bundler/datalogic_wasm_bg.js pkg/bundler/

# Copy nodejs target files
cp pkg-nodejs/datalogic_wasm_bg.wasm pkg/nodejs/
cp pkg-nodejs/datalogic_wasm.js pkg/nodejs/
cp pkg-nodejs/datalogic_wasm.d.ts pkg/nodejs/
cp pkg-nodejs/datalogic_wasm_bg.wasm.d.ts pkg/nodejs/

# Optimize WASM binaries with wasm-opt if available
if command -v wasm-opt &> /dev/null; then
    echo "Optimizing WASM binaries with wasm-opt..."
    WASM_OPT_FLAGS="-Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext"

    for target in web bundler nodejs; do
        WASM_FILE="pkg/$target/datalogic_wasm_bg.wasm"
        ORIGINAL_SIZE=$(stat -f%z "$WASM_FILE" 2>/dev/null || stat -c%s "$WASM_FILE")
        wasm-opt $WASM_OPT_FLAGS "$WASM_FILE" -o "$WASM_FILE.opt"
        mv "$WASM_FILE.opt" "$WASM_FILE"
        NEW_SIZE=$(stat -f%z "$WASM_FILE" 2>/dev/null || stat -c%s "$WASM_FILE")
        SAVED=$((ORIGINAL_SIZE - NEW_SIZE))
        echo "  $target: $ORIGINAL_SIZE -> $NEW_SIZE bytes (saved $SAVED bytes)"
    done
else
    echo "Warning: wasm-opt not found. Install binaryen for additional size optimization."
    echo "  brew install binaryen  # macOS"
    echo "  apt install binaryen   # Debian/Ubuntu"
fi

# Copy LICENSE and README
cp ../LICENSE pkg/
cp README.md pkg/

# Create package.json
cat > pkg/package.json << EOF
{
  "name": "@goplasmatic/datalogic",
  "version": "$VERSION",
  "description": "High-performance JSONLogic engine for JavaScript/TypeScript - WebAssembly powered",
  "license": "Apache-2.0",
  "repository": {
    "type": "git",
    "url": "https://github.com/GoPlasmatic/datalogic-rs"
  },
  "homepage": "https://github.com/GoPlasmatic/datalogic-rs",
  "bugs": {
    "url": "https://github.com/GoPlasmatic/datalogic-rs/issues"
  },
  "keywords": [
    "jsonlogic",
    "json",
    "logic",
    "rules",
    "rules-engine",
    "wasm",
    "webassembly",
    "business-rules"
  ],
  "type": "module",
  "main": "./nodejs/datalogic_wasm.js",
  "module": "./web/datalogic_wasm.js",
  "types": "./web/datalogic_wasm.d.ts",
  "exports": {
    ".": {
      "node": {
        "types": "./nodejs/datalogic_wasm.d.ts",
        "default": "./nodejs/datalogic_wasm.js"
      },
      "import": {
        "types": "./web/datalogic_wasm.d.ts",
        "default": "./web/datalogic_wasm.js"
      },
      "require": {
        "types": "./bundler/datalogic_wasm.d.ts",
        "default": "./bundler/datalogic_wasm.js"
      },
      "default": {
        "types": "./web/datalogic_wasm.d.ts",
        "default": "./web/datalogic_wasm.js"
      }
    },
    "./web": {
      "types": "./web/datalogic_wasm.d.ts",
      "default": "./web/datalogic_wasm.js"
    },
    "./bundler": {
      "types": "./bundler/datalogic_wasm.d.ts",
      "default": "./bundler/datalogic_wasm.js"
    },
    "./nodejs": {
      "types": "./nodejs/datalogic_wasm.d.ts",
      "default": "./nodejs/datalogic_wasm.js"
    }
  },
  "files": [
    "web/",
    "bundler/",
    "nodejs/",
    "LICENSE",
    "README.md"
  ],
  "engines": {
    "node": ">=16.0.0"
  },
  "sideEffects": false
}
EOF

# Clean up temporary directories
rm -rf pkg-web pkg-bundler pkg-nodejs

echo ""
echo "Build complete! Package created in pkg/"
echo ""
echo "To publish:"
echo "  cd pkg"
echo "  npm login --scope=@goplasmatic"
echo "  npm publish --access public"
echo ""
echo "To test locally:"
echo "  cd pkg && npm pack"
echo "  # In your test project:"
echo "  npm install /path/to/goplasmatic-datalogic-$VERSION.tgz"
