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