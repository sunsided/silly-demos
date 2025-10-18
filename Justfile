#[private]
help:
    @just --list --unsorted

# Build WASM
build *ARGS:
    @wasm-pack build {{ARGS}}

# Run WASM tests
test *ARGS:
    @wasm-pack test --headless --firefox{{ARGS}}
