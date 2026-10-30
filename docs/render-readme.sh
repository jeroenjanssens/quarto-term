#!/bin/sh
[ "$QUARTO_TERM_README" = "1" ] && exit 0
# Only regenerate the README on full renders (`quarto render docs`), not
# during `quarto preview` or when rendering individual files
[ "$QUARTO_PROJECT_RENDER_ALL" = "1" ] || exit 0
QUARTO_TERM_README=1 quarto render index.qmd --to gfm --output README.md -M title:quarto-term
sed '1s|^# quarto-term$|# quarto-term <img src="docs/logo.svg" align="right" height="139" alt="quarto-term logo" />|' _site/README.md > ../README.md
rm _site/README.md
