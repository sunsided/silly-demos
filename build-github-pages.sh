#!/bin/bash

# Build script for GitHub Pages deployment
set -e

echo "🦀 Building WASM module..."
wasm-pack build --target web

echo "📦 Installing web dependencies..."
cd web
corepack enable
yarn install --immutable

echo "🏗️  Prerendering the site for GitHub Pages..."
# The site is served from /<repository name> on GitHub Pages.
export BASE_PATH=/silly-demos
yarn build

echo "✅ Build complete! The build/ folder is ready for GitHub Pages deployment."
echo "📁 Built files are in: web/build/"

# List the contents to verify
echo "📋 Build contents:"
ls -la build/
