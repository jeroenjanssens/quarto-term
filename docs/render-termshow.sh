#!/bin/sh
for f in recording-demo.termshow examples/nyancat.termshow; do
  [ -f "$f" ] || continue
  uvx great-docs termshow render "$f" -o "termshow/$(basename "$f" .termshow)"
done
