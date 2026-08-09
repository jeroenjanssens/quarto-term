-- Test parse_cell_options function from term.lua
local h = dofile(debug.getinfo(1, "S").source:sub(2):match("(.*[/\\])") .. "helpers.lua")

local parse_cell_options = h.load_term_functions(
  {"escape_pattern", "coerce_value", "parse_cell_options"},
  { prelude = "" }
)

-- Test: basic cell options parsing
do
  local opts, code, line_options = parse_cell_options("#| echo: source\necho hello\n", "#!")
  h.assert_eq(opts["echo"], "source", "parse echo: source")
  h.assert_eq(code, "echo hello", "code after options")
  h.assert_eq(#line_options, 0, "no line options")
end

-- Test: boolean option
do
  local opts, code = parse_cell_options("#| output: false\necho hi\n", "#!")
  h.assert_eq(opts["output"], false, "output: false -> boolean false")
end

-- Test: numeric option
do
  local opts = parse_cell_options("#| timeout: 15.0\necho hi\n", "#!")
  h.assert_eq(opts["timeout"], 15.0, "timeout: 15.0 -> number")
end

-- Test: list option
do
  local opts = parse_cell_options("#| callouts: [1, 2, 3]\necho hi\n", "#!")
  h.assert_eq(type(opts["callouts"]), "table", "callouts is a table")
  h.assert_eq(opts["callouts"][1], 1, "callouts[1] = 1")
  h.assert_eq(opts["callouts"][2], 2, "callouts[2] = 2")
  h.assert_eq(opts["callouts"][3], 3, "callouts[3] = 3")
end

-- Test: trailing empty line stripped
do
  local _, code = parse_cell_options("echo hello\n\n", "#!")
  h.assert_eq(code, "echo hello", "trailing empty line stripped")
end

-- Test: line marker #! parsing
do
  local opts, code, line_options = parse_cell_options("echo hello #! hold: 1.5\n", "#!")
  h.assert_eq(code, "echo hello", "code part without marker")
  h.assert_eq(#line_options, 1, "one line option")
  h.assert_eq(line_options[1].line_index, 0, "line_index = 0")
  h.assert_eq(line_options[1].hold, 1.5, "hold = 1.5")
end

-- Test: hyphen-to-underscore normalization
do
  local _, _, line_options = parse_cell_options("cmd #! expect-prompt: false\n", "#!")
  h.assert_eq(line_options[1].expect_prompt, false, "expect-prompt -> expect_prompt")
end

-- Test: multi-option line
do
  local _, _, line_options = parse_cell_options("cmd #! literal: false, enter: true, delay: 0.2\n", "#!")
  h.assert_eq(line_options[1].literal, false, "literal: false")
  h.assert_eq(line_options[1].enter, true, "enter: true")
  h.assert_eq(line_options[1].delay, 0.2, "delay: 0.2")
end

-- Test: option block ends at first non-#| line
do
  local opts, code = parse_cell_options("#| echo: source\necho hello\n#| not-an-option: true\n", "#!")
  h.assert_eq(opts["echo"], "source", "first option parsed")
  h.assert_eq(opts["not-an-option"], nil, "non-option line not parsed as option")
  h.assert_true(code:find("#| not%-an%-option") ~= nil, "non-option line in code")
end

-- Test: custom marker
do
  local _, code, line_options = parse_cell_options("cmd #!! hold: 2.0\n", "#!!")
  h.assert_eq(code, "cmd", "code without custom marker")
  h.assert_eq(line_options[1].hold, 2.0, "custom marker parsed")
end

-- Test: different custom marker doesn't trigger on default
do
  local _, code, line_options = parse_cell_options("cmd ## hold: 2.0\n", "#!")
  h.assert_eq(#line_options, 0, "## marker doesn't match #! parser")
end

-- Test: lines without marker have no line_options entry
do
  local _, _, line_options = parse_cell_options("echo a\necho b\necho c #! hold: 1.0\n", "#!")
  h.assert_eq(#line_options, 1, "only one line has options")
  h.assert_eq(line_options[1].line_index, 2, "third line (index 2) has options")
end

-- Test: dotted key style.colorscheme
do
  local opts = parse_cell_options("#| style.colorscheme: nord\necho hi\n", "#!")
  h.assert_eq(type(opts["style"]), "table", "style is a table from dotted key")
  h.assert_eq(opts["style"]["colorscheme"], "nord", "style.colorscheme = nord")
end

-- Test: dotted key style.font-size
do
  local opts = parse_cell_options("#| style.font-size: 0.8em\necho hi\n", "#!")
  h.assert_eq(opts["style"]["font-size"], "0.8em", "style.font-size = 0.8em")
end

-- Test: multiple dotted style keys
do
  local opts = parse_cell_options("#| style.colorscheme: dracula\n#| style.font-size: 0.7em\necho hi\n", "#!")
  h.assert_eq(opts["style"]["colorscheme"], "dracula", "multi dotted: colorscheme")
  h.assert_eq(opts["style"]["font-size"], "0.7em", "multi dotted: font-size")
end

-- Test: nested block syntax for style
do
  local opts = parse_cell_options("#| style:\n#|   colorscheme: monokai\n#|   font-size: 0.9em\necho hi\n", "#!")
  h.assert_eq(type(opts["style"]), "table", "nested style is a table")
  h.assert_eq(opts["style"]["colorscheme"], "monokai", "nested style.colorscheme")
  h.assert_eq(opts["style"]["font-size"], "0.9em", "nested style.font-size")
end

-- Test: dotted key with boolean value
do
  local opts = parse_cell_options("#| style.ansi: false\necho hi\n", "#!")
  h.assert_eq(opts["style"]["ansi"], false, "style.ansi = false (boolean)")
end

-- Test: multi-level dotted key (style.html.font-size)
do
  local opts = parse_cell_options("#| style.html.font-size: 0.7em\necho hi\n", "#!")
  h.assert_eq(type(opts["style"]), "table", "multi-level: style is table")
  h.assert_eq(type(opts["style"]["html"]), "table", "multi-level: style.html is table")
  h.assert_eq(opts["style"]["html"]["font-size"], "0.7em", "style.html.font-size = 0.7em")
end

-- Test: nested block with sub-block (style.html via indentation)
do
  local opts = parse_cell_options("#| style:\n#|   colorscheme: nord\n#|   html:\n#|     font-size: 0.8em\necho hi\n", "#!")
  h.assert_eq(opts["style"]["colorscheme"], "nord", "nested sub-block: colorscheme")
  h.assert_eq(type(opts["style"]["html"]), "table", "nested sub-block: html is table")
  h.assert_eq(opts["style"]["html"]["font-size"], "0.8em", "nested sub-block: html.font-size")
end

h.finish("parse_cell_options")
function Pandoc() return nil end
