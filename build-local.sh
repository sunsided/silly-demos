#!/bin/bash

# Build script for local testing (no base path)
set -e

echo "🦀 Building WASM module..."
wasm-pack build --target web --out-dir react-app/src/pkg

echo "📦 Installing React app dependencies..."
cd react-app
npm install

echo "🏗️  Building React app for local testing..."
npm run build:local

echo "✅ Local build complete! You can now serve the dist/ folder directly."
echo "📁 Built files are in: react-app/dist/"

# List the contents to verify
echo "📋 Build contents:"
ls -la dist/

echo ""
echo "🌐 To test locally, run one of these commands:"
echo "   cd react-app/dist && python -m http.server 8001"
echo "   OR"
echo "   cd react-app && npm run preview:local"
