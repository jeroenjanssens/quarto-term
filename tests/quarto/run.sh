#!/usr/bin/env bash
# Render tests through Quarto: term cells next to Jupyter (Python) and knitr
# (R) cells. Needs quarto; the Jupyter tests need uv (a temporary venv with
# jupyter is created), the knitr tests need R with knitr. Missing tools are
# skipped with a message.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BINARY="$PROJECT_ROOT/target/debug/quarto-term"
export QUARTO_TERM_BINARY="$BINARY"

PASS=0
FAIL=0
SKIP=0

if ! command -v quarto > /dev/null; then
  echo "SKIP: quarto not found"
  exit 0
fi
if [ ! -f "$BINARY" ]; then
  echo "ERROR: binary not found at $BINARY (run 'cargo build' first)"
  exit 1
fi

WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT
mkdir -p "$WORK_DIR/_extensions"
cp -r "$PROJECT_ROOT/_extensions/term" "$WORK_DIR/_extensions/"

# page TITLE ENGINE_CELL FENCE [FRONT_MATTER]: a document with an engine cell
# and term cells (a hidden setup cell, and a cell with a callout that uses its
# variable)
page() {
  printf -- '---
title: "%s"
%s
filters: [term]
extensions:
  term:
    shell: bash
    shell-args: ["--norc", "--noprofile"]
    cache: false
    timeout: 10
---

%s

%s
#| include: false
export SETUP="setup-$((40 + 2))"
```

%s
#| callout: [1]
echo "term sees $SETUP"
```
' "$1" "${4:-}" "$2" "$3" "$3"
}

# render FILE: render to HTML, printing the plain text of all <pre> blocks
# followed by quarto's stderr
render() {
  (cd "$WORK_DIR" && quarto render "$1" --to html 2> "$1.log" > /dev/null) || true
  python3 - "$WORK_DIR/${1%.qmd}.html" <<'PY' || true
import html, re, sys
try:
    s = open(sys.argv[1]).read()
except OSError:
    sys.exit()
for p in re.findall(r"<pre.*?</pre>", s, re.S):
    print(html.unescape(re.sub(r"<[^>]+>", "", p)))
PY
  cat "$WORK_DIR/$1.log"
}

check() {
  local name="$1" output="$2"
  shift 2
  for expected in "$@"; do
    if ! printf '%s' "$output" | grep -qF -- "$expected"; then
      echo "  FAIL: $name"
      echo "    expected '$expected', got: $(printf '%s' "$output" | head -c 600)"
      FAIL=$((FAIL + 1))
      return
    fi
  done
  echo "  PASS: $name"
  PASS=$((PASS + 1))
}

echo "=== quarto-term Quarto render tests ==="

echo ""
echo "Jupyter:"
if command -v uv > /dev/null; then
  uv venv -q "$WORK_DIR/.venv"
  uv pip install -q --python "$WORK_DIR/.venv/bin/python" jupyter pyyaml
  export QUARTO_PYTHON="$WORK_DIR/.venv/bin/python"
  page "Jupyter" '```{python}
print(f"python-{6 * 7}")
```' '```{term}' > "$WORK_DIR/jupyter.qmd"
  out=$(render jupyter.qmd)
  check "Jupyter + {term}: both run, cell options kept" "$out" \
    "python-42" "term sees setup-42" '$SETUP" <1>'
else
  echo "  SKIP: uv not found"
  SKIP=$((SKIP + 1))
fi

echo ""
echo "knitr:"
if command -v Rscript > /dev/null && Rscript -e 'library(knitr)' > /dev/null 2>&1; then
  page "knitr" '```{r}
cat(paste0("r-", 6 * 7), "\n")
```' '```{.term}' > "$WORK_DIR/knitr-dotted.qmd"
  out=$(render knitr-dotted.qmd)
  check "knitr + {.term}: both run, cell options kept" "$out" \
    "r-42" "term sees setup-42" '$SETUP" <1>'

  page "knitr" '```{r}
cat("r runs\n")
```' '```{term}' > "$WORK_DIR/knitr-braces.qmd"
  out=$(render knitr-braces.qmd)
  check "knitr + {term}: warns to use {.term}" "$out" "consumed the" "{.term}"
else
  echo "  SKIP: R with knitr not found"
  SKIP=$((SKIP + 1))
fi

echo ""
echo "No engine:"
page "No engine" '' '```{.term}' > "$WORK_DIR/dotted.qmd"
out=$(render dotted.qmd)
check "only {.term} cells: no engine needed" "$out" "term sees setup-42" '$SETUP" <1>'

page "engine: markdown" '' '```{term}' 'engine: markdown' > "$WORK_DIR/braces.qmd"
out=$(render braces.qmd)
check "only {term} cells with engine: markdown" "$out" "term sees setup-42" '$SETUP" <1>'

echo ""
echo "==========================="
echo "Results: $PASS passed, $FAIL failed, $SKIP skipped"
[ "$FAIL" -eq 0 ]
