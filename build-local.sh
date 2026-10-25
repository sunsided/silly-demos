#!/bin/bash

# Build script for local testing (no base path)
set -e

echo "🦀 Building WASM module..."
wasm-pack build --target web

echo "📦 Installing web dependencies..."
cd web
corepack enable
yarn install --immutable

echo "🏗️  Prerendering the site for local testing..."
yarn build

echo "✅ Local build complete! You can now serve the build/ folder directly."
echo "📁 Built files are in: web/build/"

# List the contents to verify
echo "📋 Build contents:"
ls -la build/

echo ""
echo "🌐 To test locally, run one of these commands:"
echo "   cd web/build && python -m http.server 8001"
echo "   OR"
echo "   cd web && yarn preview"
