#!/bin/sh
# Use the latest commit on the main branch of Great Docs.
great_docs="git+https://github.com/posit-dev/great-docs"
for f in recording-demo.termshow examples/nyancat.termshow; do
  [ -f "$f" ] || continue
  uvx --refresh-package great-docs --from "$great_docs" \
    great-docs termshow render "$f" -o "termshow/$(basename "$f" .termshow)"
done
