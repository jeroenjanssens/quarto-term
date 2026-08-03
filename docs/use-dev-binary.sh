#!/bin/sh
# Copy local dev binary for docs rendering (skipped if not present)
cd "$(dirname "$0")"
src="../target/release/quarto-term"
dst="_extensions/term/bin/quarto-term-aarch64-apple-darwin"
if [ -f "$src" ]; then
  mkdir -p "$(dirname "$dst")"
  cp "$src" "$dst"
fi
