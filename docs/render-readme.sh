#!/bin/sh
[ "$QUARTO_TERM_README" = "1" ] && exit 0
QUARTO_TERM_README=1 quarto render index.qmd --to gfm --output README.md
mv _site/README.md ../README.md
