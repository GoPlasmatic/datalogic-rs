#!/bin/bash
cd examples/web
wasm-pack build --target web --out-dir ../../docs/pkg
rm ../../docs/pkg/.gitignore