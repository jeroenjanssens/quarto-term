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

# term_page TITLE CODE: a document with one term cell and a paragraph after it
term_page() {
  printf -- '---
title: "%s"
engine: markdown
filters: [term]
keep-typ: true
extensions:
  term:
    shell: bash
    shell-args: ["--norc", "--noprofile"]
    cache: false
    timeout: 20
---

```{term}
%s
```

TEXT-AFTER-THE-CELL
' "$1" "$2"
}

# render_to FILE FORMAT: render FILE to FORMAT and print the output file
# (for typst, the .typ source; quarto also compiles it to PDF)
render_to() {
  local base="${1%.qmd}" out
  case "$2" in
    typst) out="$base.typ" ;;
    *) out="$base-$2.out" ;;
  esac
  if [ "$2" = typst ]; then
    (cd "$WORK_DIR" && quarto render "$1" --to typst 2> "$1.$2.log" > /dev/null) || true
  else
    (cd "$WORK_DIR" && quarto render "$1" --to "$2" -o "$base-$2.out" 2> "$1.$2.log" > /dev/null) || true
  fi
  cat "$WORK_DIR/$out" 2> /dev/null || true
}

# check_absent NAME OUTPUT TEXT...: none of TEXT may occur in OUTPUT
check_absent() {
  local name="$1" output="$2"
  shift 2
  for unexpected in "$@"; do
    if printf '%s' "$output" | grep -qF -- "$unexpected"; then
      echo "  FAIL: $name"
      echo "    unexpected '$unexpected' in: $(printf '%s' "$output" | head -c 600)"
      FAIL=$((FAIL + 1))
      return
    fi
  done
  echo "  PASS: $name"
  PASS=$((PASS + 1))
}

echo ""
echo "Shortcode-like output:"
# The cell's source must not contain an unclosed `{{<` itself (Quarto reads
# the source before any filter runs), so the cell builds it with printf
SHORTCODE_CODE="printf 'alice{{\\e[32m2\\e[0m,3}}.txt\\n'
echo 'closed {{< meta title >}} end'
printf 'unclosed {{%s end\\n' '<'"
term_page "Shortcodes" "$SHORTCODE_CODE" > "$WORK_DIR/shortcodes.qmd"
# Without colors, Typst output is a raw block (the #raw() string path)
term_page "Shortcodes" "echo 'closed {{< meta title >}} end'
printf 'unclosed {{%s end\\n' '<'" > "$WORK_DIR/shortcodes-plain.qmd"

out=$(render shortcodes.qmd)
check "html: {{< in output doesn't start a shortcode" "$out" \
  'alice{{2,3}}.txt' 'closed {{< meta title >}} end' 'unclosed {{< end'
# The paragraph must come after the cell's <pre>, not inside it
after_pre=$(python3 - "$WORK_DIR/shortcodes.html" <<'PY' || true
import sys
s = open(sys.argv[1]).read()
i = s.find("<p>TEXT-AFTER-THE-CELL</p>")
print("outside" if i > 0 and s.rfind("</pre>", 0, i) > s.rfind("<pre", 0, i) else "inside")
PY
)
check "html: the rest of the page is kept" "$after_pre" "outside"
check_absent "html: shortcodes in output aren't expanded" "$out" 'closed Shortcodes end'

for fmt in gfm commonmark markdown; do
  out=$(render_to shortcodes.qmd "$fmt")
  check "$fmt: {{< in output doesn't start a shortcode" "$out" \
    'alice{{2,3}}.txt' 'closed {{< meta title >}} end' 'unclosed {{< end' 'TEXT-AFTER-THE-CELL'
  check_absent "$fmt: shortcodes in output aren't expanded" "$out" 'closed Shortcodes end'
done

# (Pandoc turns `&#123;` back into `{` once shortcodes are processed)
out=$(render_to shortcodes.qmd revealjs)
check "revealjs: {{< in output doesn't start a shortcode" "$out" \
  'alice{{<span' 'closed {{&lt; meta title &gt;}} end' 'unclosed {{&lt; end' '<p>TEXT-AFTER-THE-CELL</p>'

out=$(render_to shortcodes.qmd latex)
check "latex: braces are escaped, rest of the page kept" "$out" \
  'closed \{\{< meta title >\}\} end' 'unclosed \{\{< end' 'TEXT-AFTER-THE-CELL'

out=$(render_to shortcodes.qmd typst)
check "typst (colored): escaped markup, rest of the page kept" "$out" \
  'closed {{\< meta title \>}} end' 'unclosed {{\< end' 'TEXT-AFTER-THE-CELL'
out=$(render_to shortcodes-plain.qmd typst)
check "typst (plain): #raw() string, rest of the page kept" "$out" \
  '#raw(block: true, ' 'closed {{\u{3c} meta title >}} end' 'unclosed {{\u{3c} end' 'TEXT-AFTER-THE-CELL'
if [ -s "$WORK_DIR/shortcodes-plain.pdf" ]; then
  echo "  PASS: typst (plain): compiles to PDF"
  PASS=$((PASS + 1))
else
  echo "  FAIL: typst (plain): no PDF ($(tail -3 "$WORK_DIR/shortcodes-plain.qmd.typst.log"))"
  FAIL=$((FAIL + 1))
fi

echo ""
echo "Multibyte output:"
# ~60 KB written at once by cat (like bat), so PTY reads split characters
# (with 0.7.1 on every run; output from many small writes splits only sometimes)
MULTIBYTE_CODE="f=\$(mktemp); l=\$(printf '─%.0s' {1..60})' ✓é😀'; for i in {1..300}; do echo \"\$l\"; done > \"\$f\"; cat \"\$f\"; rm \"\$f\""
term_page "Multibyte" "$MULTIBYTE_CODE" > "$WORK_DIR/multibyte.qmd"
out=$(render multibyte.qmd)
check "html: box drawing, accents, and emoji are intact" "$out" \
  "$(printf '─%.0s' $(seq 60)) ✓é😀"
check_absent "html: no replacement characters" "$out" '�'

echo ""
echo "Invisible zero-width characters:"
# A BOM at the start of a line that fits the terminal exactly must not make it wrap
BOM_CODE="printf '\\xef\\xbb\\xbf%s\\n' \$(printf 'x%.0s' {1..80})"
term_page "BOM" "$BOM_CODE" > "$WORK_DIR/bom.qmd"
out=$(render bom.qmd)
check "html: a line with a BOM and 80 columns stays one line" "$out" "$(printf 'x%.0s' $(seq 80))"
wrapped=$(printf '%s\n' "$out" | grep -cx 'x' || true)
check "html: nothing wraps" "wrapped lines: $wrapped" "wrapped lines: 0"

echo ""
echo "==========================="
echo "Results: $PASS passed, $FAIL failed, $SKIP skipped"
[ "$FAIL" -eq 0 ]
