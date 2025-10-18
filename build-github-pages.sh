#!/bin/bash

# Build script for GitHub Pages deployment
set -e

echo "🦀 Building WASM module..."
wasm-pack build --target web --out-dir react-app/src/pkg

echo "📦 Installing React app dependencies..."
cd react-app
npm install

echo "🏗️  Building React app for GitHub Pages..."
# Set the base path to the repository name for GitHub Pages deployment
export VITE_BASE_PATH=/silly-demos
npm run build

echo "✅ Build complete! The dist/ folder is ready for GitHub Pages deployment."
echo "📁 Built files are in: react-app/dist/"

# List the contents to verify
echo "📋 Build contents:"
ls -la dist/
