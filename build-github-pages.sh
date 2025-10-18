#!/bin/bash

# Build script for GitHub Pages deployment
set -e

echo "🦀 Building WASM module..."
wasm-pack build --target web --out-dir react-app/src/pkg

echo "📦 Installing React app dependencies..."
cd react-app
npm install

echo "🏗️  Building React app for GitHub Pages..."
npm run build:github-pages

echo "✅ Build complete! The dist/ folder is ready for GitHub Pages deployment."
echo "📁 Built files are in: react-app/dist/"

# List the contents to verify
echo "📋 Build contents:"
ls -la dist/
