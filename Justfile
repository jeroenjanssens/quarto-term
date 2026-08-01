set shell := ["bash", "-euo", "pipefail", "-c"]

# Run all tests
test: test-rust test-lua test-deno test-e2e

# Rust unit and integration tests
test-rust:
    cargo test

# Lua filter tests
test-lua: build
    for f in tests/lua/test_*.lua; do \
        echo "Running $f..."; \
        pandoc --lua-filter "$f" /dev/null -t plain 2>&1 || exit 1; \
    done

# Deno TypeScript tests
test-deno:
    deno test tests/deno/ --allow-read --allow-write --allow-env

# End-to-end tests (requires built binary)
test-e2e: build
    bash tests/e2e/run_e2e.sh

# Build debug binary
build:
    cargo build
