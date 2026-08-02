-- Test parse_cell_options function from term.lua
local pass_count = 0
local fail_count = 0

local function assert_eq(actual, expected, msg)
  if actual == expected then
    pass_count = pass_count + 1
  else
    fail_count = fail_count + 1
    io.stderr:write(string.format("FAIL: %s\n  expected: %s (%s)\n  actual:   %s (%s)\n",
      msg, tostring(expected), type(expected), tostring(actual), type(actual)))
  end
end

local function assert_true(val, msg)
  assert_eq(val, true, msg)
end

-- Load the functions from term.lua
local filter_dir = debug.getinfo(1, "S").source:sub(2):match("(.*[/\\])") or "./"
local project_root = filter_dir:match("(.*/?)tests/lua/") or "./"
local term_lua = project_root .. "_extensions/term/term.lua"

local f = io.open(term_lua, "r")
if not f then
  io.stderr:write("ERROR: cannot open " .. term_lua .. "\n")
  os.exit(1)
end
local source = f:read("*a")
f:close()

-- Extract helper functions needed
local escape_pattern_src = source:match("(local function escape_pattern.-\nend)")
local coerce_value_src = source:match("(local function coerce_value.-\nend)")
local parse_cell_options_src = source:match("(local function parse_cell_options.-\nend)")

local chunk = escape_pattern_src .. "\n" .. coerce_value_src .. "\n" .. parse_cell_options_src ..
  "\nreturn parse_cell_options"
local parse_cell_options = load(chunk)()

-- Test: basic cell options parsing
do
  local opts, code, line_options = parse_cell_options("#| echo: source\necho hello\n", "#!")
  assert_eq(opts["echo"], "source", "parse echo: source")
  assert_eq(code, "echo hello", "code after options")
  assert_eq(#line_options, 0, "no line options")
end

-- Test: boolean option
do
  local opts, code = parse_cell_options("#| output: false\necho hi\n", "#!")
  assert_eq(opts["output"], false, "output: false -> boolean false")
end

-- Test: numeric option
do
  local opts = parse_cell_options("#| timeout: 15.0\necho hi\n", "#!")
  assert_eq(opts["timeout"], 15.0, "timeout: 15.0 -> number")
end

-- Test: list option
do
  local opts = parse_cell_options("#| callouts: [1, 2, 3]\necho hi\n", "#!")
  assert_eq(type(opts["callouts"]), "table", "callouts is a table")
  assert_eq(opts["callouts"][1], 1, "callouts[1] = 1")
  assert_eq(opts["callouts"][2], 2, "callouts[2] = 2")
  assert_eq(opts["callouts"][3], 3, "callouts[3] = 3")
end

-- Test: trailing empty line stripped
do
  local _, code = parse_cell_options("echo hello\n\n", "#!")
  assert_eq(code, "echo hello", "trailing empty line stripped")
end

-- Test: line marker #! parsing
do
  local opts, code, line_options = parse_cell_options("echo hello #! hold: 1.5\n", "#!")
  assert_eq(code, "echo hello", "code part without marker")
  assert_eq(#line_options, 1, "one line option")
  assert_eq(line_options[1].line_index, 0, "line_index = 0")
  assert_eq(line_options[1].hold, 1.5, "hold = 1.5")
end

-- Test: hyphen-to-underscore normalization
do
  local _, _, line_options = parse_cell_options("cmd #! expect-prompt: false\n", "#!")
  assert_eq(line_options[1].expect_prompt, false, "expect-prompt -> expect_prompt")
end

-- Test: multi-option line
do
  local _, _, line_options = parse_cell_options("cmd #! literal: false, enter: true, delay: 0.2\n", "#!")
  assert_eq(line_options[1].literal, false, "literal: false")
  assert_eq(line_options[1].enter, true, "enter: true")
  assert_eq(line_options[1].delay, 0.2, "delay: 0.2")
end

-- Test: option block ends at first non-#| line
do
  local opts, code = parse_cell_options("#| echo: source\necho hello\n#| not-an-option: true\n", "#!")
  assert_eq(opts["echo"], "source", "first option parsed")
  assert_eq(opts["not-an-option"], nil, "non-option line not parsed as option")
  assert_true(code:find("#| not%-an%-option") ~= nil, "non-option line in code")
end

-- Test: custom marker
do
  local _, code, line_options = parse_cell_options("cmd #!! hold: 2.0\n", "#!!")
  assert_eq(code, "cmd", "code without custom marker")
  assert_eq(line_options[1].hold, 2.0, "custom marker parsed")
end

-- Test: different custom marker doesn't trigger on default
do
  local _, code, line_options = parse_cell_options("cmd ## hold: 2.0\n", "#!")
  -- With #! as marker, ## won't match
  assert_eq(#line_options, 0, "## marker doesn't match #! parser")
end

-- Test: lines without marker have no line_options entry
do
  local _, _, line_options = parse_cell_options("echo a\necho b\necho c #! hold: 1.0\n", "#!")
  assert_eq(#line_options, 1, "only one line has options")
  assert_eq(line_options[1].line_index, 2, "third line (index 2) has options")
end

-- Test: dotted key style.colorscheme
do
  local opts = parse_cell_options("#| style.colorscheme: nord\necho hi\n", "#!")
  assert_eq(type(opts["style"]), "table", "style is a table from dotted key")
  assert_eq(opts["style"]["colorscheme"], "nord", "style.colorscheme = nord")
end

-- Test: dotted key style.font-size
do
  local opts = parse_cell_options("#| style.font-size: 0.8em\necho hi\n", "#!")
  assert_eq(opts["style"]["font-size"], "0.8em", "style.font-size = 0.8em")
end

-- Test: multiple dotted style keys
do
  local opts = parse_cell_options("#| style.colorscheme: dracula\n#| style.font-size: 0.7em\necho hi\n", "#!")
  assert_eq(opts["style"]["colorscheme"], "dracula", "multi dotted: colorscheme")
  assert_eq(opts["style"]["font-size"], "0.7em", "multi dotted: font-size")
end

-- Test: nested block syntax for style
do
  local opts = parse_cell_options("#| style:\n#|   colorscheme: monokai\n#|   font-size: 0.9em\necho hi\n", "#!")
  assert_eq(type(opts["style"]), "table", "nested style is a table")
  assert_eq(opts["style"]["colorscheme"], "monokai", "nested style.colorscheme")
  assert_eq(opts["style"]["font-size"], "0.9em", "nested style.font-size")
end

-- Test: dotted key with boolean value
do
  local opts = parse_cell_options("#| style.ansi: false\necho hi\n", "#!")
  assert_eq(opts["style"]["ansi"], false, "style.ansi = false (boolean)")
end

-- Test: multi-level dotted key (style.html.font-size)
do
  local opts = parse_cell_options("#| style.html.font-size: 0.7em\necho hi\n", "#!")
  assert_eq(type(opts["style"]), "table", "multi-level: style is table")
  assert_eq(type(opts["style"]["html"]), "table", "multi-level: style.html is table")
  assert_eq(opts["style"]["html"]["font-size"], "0.7em", "style.html.font-size = 0.7em")
end

-- Test: nested block with sub-block (style.html via indentation)
do
  local opts = parse_cell_options("#| style:\n#|   colorscheme: nord\n#|   html:\n#|     font-size: 0.8em\necho hi\n", "#!")
  assert_eq(opts["style"]["colorscheme"], "nord", "nested sub-block: colorscheme")
  assert_eq(type(opts["style"]["html"]), "table", "nested sub-block: html is table")
  assert_eq(opts["style"]["html"]["font-size"], "0.8em", "nested sub-block: html.font-size")
end

-- Report
io.stderr:write(string.format("\nparse_cell_options: %d passed, %d failed\n", pass_count, fail_count))
if fail_count > 0 then
  os.exit(1)
end

function Pandoc() return nil end
