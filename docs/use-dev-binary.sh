#!/bin/sh
# Copy local dev binary for docs rendering (skipped if not present)
src="../target/release/quarto-term"
dst="_extensions/term/bin/quarto-term-aarch64-apple-darwin"
if [ -f "$src" ]; then
  cp "$src" "$dst"
fi
