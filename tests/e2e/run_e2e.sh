#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
BINARY="$PROJECT_ROOT/target/debug/quarto-term"
FILTER="$PROJECT_ROOT/_extensions/term/term.lua"

if [ ! -f "$BINARY" ]; then
  echo "ERROR: binary not found at $BINARY (run 'cargo build' first)"
  exit 1
fi

export PATH="$(dirname "$BINARY"):$PATH"

PASS=0
FAIL=0

METADATA='---
extensions:
  term:
    shell: bash
    timeout: 5.0
---

'

run_test() {
  local name="$1"
  local input="$2"
  local expected="$3"
  local output

  output=$(echo "${METADATA}${input}" | timeout 15 pandoc -f markdown -t html --lua-filter "$FILTER" 2>/dev/null) || true

  if echo "$output" | grep -qF "$expected"; then
    echo "  PASS: $name"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $name"
    echo "    expected: '$expected'"
    echo "    got: $(echo "$output" | head -5)"
    FAIL=$((FAIL + 1))
  fi
}

run_test_absent() {
  local name="$1"
  local input="$2"
  local absent="$3"
  local output

  output=$(echo "${METADATA}${input}" | timeout 15 pandoc -f markdown -t html --lua-filter "$FILTER" 2>/dev/null) || true

  if ! echo "$output" | grep -qF "$absent"; then
    echo "  PASS: $name"
    PASS=$((PASS + 1))
  else
    echo "  FAIL: $name (should NOT contain '$absent')"
    echo "    got: $(echo "$output" | head -5)"
    FAIL=$((FAIL + 1))
  fi
}

echo "=== quarto-term E2E tests ==="
echo ""

# --- Basic output ---
echo "Basic output:"

run_test "echo hello produces output" \
  '``` {.term}
echo hello
```' \
  "hello"

run_test "term-output class present" \
  '``` {.term}
echo hi
```' \
  "term-output"

# --- State persistence ---
echo ""
echo "State persistence:"

run_test "variable persists between cells" \
  '``` {.term}
export E2E_VAR=success42
```

``` {.term}
echo $E2E_VAR
```' \
  "success42"

# --- Echo modes ---
echo ""
echo "Echo modes:"

run_test "echo: source shows term-source class" \
  '``` {.term}
#| echo: source
echo hello
```' \
  "term-source"

run_test "echo: false produces no output" \
  '``` {.term}
#| echo: false
#| output: false
echo hidden
```' \
  ""

# --- Annotations ---
echo ""
echo "Annotations:"

run_test "callouts add term-callout span" \
  '``` {.term}
#| callouts: [1]
echo annotated
```' \
  "term-callout"

run_test_absent "remove strips matching line" \
  '``` {.term}
#| remove: ["REMOVEME"]
echo KEEPME
echo REMOVEME
echo KEEPALSO
```' \
  "REMOVEME"

# --- Special characters ---
echo ""
echo "Special characters:"

run_test "HTML special chars are escaped" \
  '``` {.term}
echo "<b>&test</b>"
```' \
  "&amp;"

# --- Line options ---
echo ""
echo "Line options:"

run_test "expect-prompt: false doesn't timeout" \
  '``` {.term}
sleep 0.1 #! expect-prompt: false, hold: 0.3
echo done
```' \
  "done"

# --- Summary ---
echo ""
echo "==========================="
echo "Results: $PASS passed, $FAIL failed"

if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
