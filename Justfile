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

# End-to-end tests including Docker mode (requires a running Docker daemon)
test-e2e-docker: build
    QUARTO_TERM_TEST_DOCKER=1 bash tests/e2e/run_e2e.sh

# Render tests through Quarto with Jupyter and knitr (needs quarto; uv and R/knitr optional)
test-quarto: build
    bash tests/quarto/run.sh

# Build debug binary
build:
    cargo build

# Preview the website locally with live reload
preview:
    cargo build --release
    quarto preview docs

# Release: bump patch version (0.4.0 → 0.4.1), commit, tag, push
release-patch:
    #!/usr/bin/env bash
    set -euo pipefail
    current=$(grep '^version:' _extensions/term/_extension.yml | sed 's/version: //')
    IFS='.' read -r major minor patch <<< "$current"
    new="${major}.${minor}.$((patch + 1))"
    sed -i '' "s/^version: .*/version: ${new}/" _extensions/term/_extension.yml
    sed -i '' "s/^version = .*/version = \"${new}\"/" Cargo.toml
    sed -i '' "s/^local VERSION = .*/local VERSION = \"${new}\"/" _extensions/term/term.lua
    cargo update --workspace --offline --quiet
    git add _extensions/term/_extension.yml Cargo.toml Cargo.lock _extensions/term/term.lua
    git commit -m "Bump version to ${new}"
    git tag "v${new}"
    git push origin main "v${new}"
    echo "Released v${new}"

# Release: bump minor version (0.4.1 → 0.5.0), commit, tag, push
release-minor:
    #!/usr/bin/env bash
    set -euo pipefail
    current=$(grep '^version:' _extensions/term/_extension.yml | sed 's/version: //')
    IFS='.' read -r major minor patch <<< "$current"
    new="${major}.$((minor + 1)).0"
    sed -i '' "s/^version: .*/version: ${new}/" _extensions/term/_extension.yml
    sed -i '' "s/^version = .*/version = \"${new}\"/" Cargo.toml
    sed -i '' "s/^local VERSION = .*/local VERSION = \"${new}\"/" _extensions/term/term.lua
    cargo update --workspace --offline --quiet
    git add _extensions/term/_extension.yml Cargo.toml Cargo.lock _extensions/term/term.lua
    git commit -m "Bump version to ${new}"
    git tag "v${new}"
    git push origin main "v${new}"
    echo "Released v${new}"

# Release: bump major version (0.4.1 → 1.0.0), commit, tag, push
release-major:
    #!/usr/bin/env bash
    set -euo pipefail
    current=$(grep '^version:' _extensions/term/_extension.yml | sed 's/version: //')
    IFS='.' read -r major minor patch <<< "$current"
    new="$((major + 1)).0.0"
    sed -i '' "s/^version: .*/version: ${new}/" _extensions/term/_extension.yml
    sed -i '' "s/^version = .*/version = \"${new}\"/" Cargo.toml
    sed -i '' "s/^local VERSION = .*/local VERSION = \"${new}\"/" _extensions/term/term.lua
    cargo update --workspace --offline --quiet
    git add _extensions/term/_extension.yml Cargo.toml Cargo.lock _extensions/term/term.lua
    git commit -m "Bump version to ${new}"
    git tag "v${new}"
    git push origin main "v${new}"
    echo "Released v${new}"
