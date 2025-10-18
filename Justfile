# [private]
help:
    @just --list --unsorted

# Build WASM
build *ARGS:
    @wasm-pack build --target=web {{ ARGS }}

# Run WASM tests
test *ARGS:
    @wasm-pack test --headless --firefox{{ ARGS }}

# Serve the current directory on a given port
serve PORT="8000":
    python3 -m http.server {{PORT}}

# React App Commands

# Setup React app dependencies
react-setup:
    cd react-app && yarn install

# Start React development server
react-dev:
    cd react-app && yarn dev

# Build React app for production
react-build:
    cd react-app && yarn build

# Preview React production build
react-preview:
    cd react-app && yarn preview

# Run React TypeScript checks
react-lint:
    cd react-app && yarn lint

# Copy WASM package to React app
react-copy-wasm:
    cp -r pkg react-app/src/

# Full React development setup (build WASM + copy + install + dev)
react-full-dev:
    @just build
    @just react-copy-wasm
    @just react-setup
    @just react-dev

# Full React production setup (build WASM + copy + install + build)
react-full-build:
    @just build
    @just react-copy-wasm
    @just react-setup
    @just react-build

# Clean React build artifacts
react-clean:
    cd react-app && rm -rf dist node_modules yarn.lock src/pkg