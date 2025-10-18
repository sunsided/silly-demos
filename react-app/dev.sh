#!/bin/bash

# Development helper script for WASM Silly Demos React App

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

echo "🦀 WASM Silly Demos Development Helper"
echo "======================================"

case "${1:-help}" in
  "setup")
    echo "📦 Setting up the React application..."
    cd "$SCRIPT_DIR"
    yarn install
    echo "✅ Setup complete! Run 'yarn dev' to start the development server."
    ;;
  
  "dev")
    echo "🚀 Starting development server..."
    cd "$SCRIPT_DIR"
    yarn dev
    ;;
  
  "build")
    echo "🏗️  Building for production..."
    cd "$SCRIPT_DIR"
    yarn build
    echo "✅ Build complete! Check the 'dist' directory."
    ;;
  
  "check-wasm")
    echo "🔍 Checking WASM package..."
    if [ -f "$SCRIPT_DIR/public/pkg/silly_demos.js" ]; then
      echo "✅ WASM package found at public/pkg/"
    else
      echo "❌ WASM package not found!"
      echo "Make sure to build the Rust project and copy pkg/ to public/"
      exit 1
    fi
    ;;
  
  "copy-wasm")
    echo "📋 Copying WASM package from ../pkg..."
    if [ -d "$ROOT_DIR/pkg" ]; then
      cp -r "$ROOT_DIR/pkg" "$SCRIPT_DIR/public/"
      echo "✅ WASM package copied successfully!"
    else
      echo "❌ WASM package not found at ../pkg/"
      echo "Build the Rust project first with: just build-wasm"
      exit 1
    fi
    ;;
  
  "lint")
    echo "🔧 Running TypeScript type checking..."
    cd "$SCRIPT_DIR"
    yarn lint
    echo "✅ Type checking complete!"
    ;;
  
  "clean")
    echo "🧹 Cleaning build artifacts..."
    cd "$SCRIPT_DIR"
    rm -rf dist node_modules/.vite
    echo "✅ Clean complete!"
    ;;
  
  "help"|*)
    echo "Available commands:"
    echo "  setup      - Install dependencies"
    echo "  dev        - Start development server"
    echo "  build      - Build for production"
    echo "  check-wasm - Check if WASM package is available"
    echo "  copy-wasm  - Copy WASM package from ../pkg"
    echo "  lint       - Run TypeScript type checking"
    echo "  clean      - Clean build artifacts"
    echo "  help       - Show this help message"
    ;;
esac
