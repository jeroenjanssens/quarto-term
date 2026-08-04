#!/bin/sh
# Copy local dev binary for docs rendering (skipped if not present)
cd "$(dirname "$0")"
src="../target/release/quarto-term"
dst="_extensions/term/bin/quarto-term-aarch64-apple-darwin"
if [ -f "$src" ]; then
  mkdir -p "$(dirname "$dst")"
  cp "$src" "$dst"
  # Write a version marker so install-binary.ts skips downloading
  version=$(grep '^version:' _extensions/term/_extension.yml | awk '{print $2}')
  echo "$version" > _extensions/term/bin/.version
fi
