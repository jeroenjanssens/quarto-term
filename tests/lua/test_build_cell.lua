-- Test build_cell function from term.lua
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

local function assert_nil(val, msg)
  if val == nil then
    pass_count = pass_count + 1
  else
    fail_count = fail_count + 1
    io.stderr:write(string.format("FAIL: %s\n  expected nil, got: %s (%s)\n",
      msg, tostring(val), type(val)))
  end
end

-- Load functions from term.lua
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

-- Extract needed functions
local escape_pattern_src = source:match("(local function escape_pattern.-\nend)")
local coerce_value_src = source:match("(local function coerce_value.-\nend)")
local as_list_src = source:match("(local function as_list.-\nend)")
local parse_cell_options_src = source:match("(local function parse_cell_options.-\nend)")
local build_cell_src = source:match("(local function build_cell.-\nend)")

local chunk = [[
local pandoc = { List = function(t) return t or {} end }
]] .. escape_pattern_src .. "\n" .. coerce_value_src .. "\n" .. as_list_src .. "\n" .. parse_cell_options_src .. "\n" .. build_cell_src ..
  "\nreturn build_cell"
local build_cell = load(chunk)()

-- Helper: make a mock block
local function mock_block(text)
  return { text = text, classes = {"term"} }
end

-- Test: default options
do
  local cell = build_cell(mock_block("echo hi\n"), 1, {})
  assert_eq(cell.options.echo, nil, "default echo = nil (Rust defaults to terminal)")
  assert_eq(cell.options.output, true, "default output = true")
  assert_eq(cell.options.fullscreen, false, "default fullscreen = false")
  assert_eq(cell.options.keep_last_prompt, nil, "default keep_last_prompt = nil (Rust defaults to false)")
  assert_eq(cell.options.highlight, nil, "default highlight = nil (Rust defaults to bash)")
  assert_eq(cell.id, 1, "cell id = 1")
  assert_eq(cell.code, "echo hi", "code parsed")
end

-- Test: include: false
do
  local cell = build_cell(mock_block("#| include: false\necho hidden\n"), 1, {})
  assert_eq(cell.options.echo, "false", "include:false -> echo:false")
  assert_eq(cell.options.output, false, "include:false -> output:false")
  assert_eq(cell._include, false, "_include = false")
end

-- Test: echo: source
do
  local cell = build_cell(mock_block("#| echo: source\necho hi\n"), 1, {})
  assert_eq(cell.options.echo, "source", "echo: source")
end

-- Test: echo: true (boolean)
do
  local cell = build_cell(mock_block("#| echo: true\necho hi\n"), 1, {})
  assert_eq(cell.options.echo, true, "echo: true preserved as boolean")
end

-- Test: echo: false
do
  local cell = build_cell(mock_block("#| echo: false\necho hi\n"), 1, {})
  assert_eq(cell.options.echo, false, "echo: false")
end

-- Test: output: false
do
  local cell = build_cell(mock_block("#| output: false\necho hi\n"), 1, {})
  assert_eq(cell.options.output, false, "output: false")
end

-- Test: fullscreen: true
do
  local cell = build_cell(mock_block("#| fullscreen: true\necho hi\n"), 1, {})
  assert_eq(cell.options.fullscreen, true, "fullscreen: true")
end

-- Test: scroll: false
do
  local cell = build_cell(mock_block("#| scroll: false\necho hi\n"), 1, {})
  assert_eq(cell.options.scroll, false, "scroll: false")
end

-- Test: keep-last-prompt (kebab-case)
do
  local cell = build_cell(mock_block("#| keep-last-prompt: true\necho hi\n"), 1, {})
  assert_eq(cell.options.keep_last_prompt, true, "keep-last-prompt kebab")
end

-- Test: keep_last_prompt (snake_case)
do
  local cell = build_cell(mock_block("#| keep_last_prompt: true\necho hi\n"), 1, {})
  assert_eq(cell.options.keep_last_prompt, true, "keep_last_prompt snake")
end

-- Test: style.ansi: false (dotted syntax)
do
  local cell = build_cell(mock_block("#| style.ansi: false\necho hi\n"), 1, {})
  assert_eq(cell.options.ansi, false, "style.ansi: false")
end

-- Test: spacing: true
do
  local cell = build_cell(mock_block("#| spacing: true\necho hi\n"), 1, {})
  assert_eq(cell.options.spacing, true, "spacing: true")
end

-- Test: typing: false
do
  local cell = build_cell(mock_block("#| typing: false\necho hi\n"), 1, {})
  assert_eq(cell.options.typing, false, "typing: false")
end

-- Test: typing: true
do
  local cell = build_cell(mock_block("#| typing: true\necho hi\n"), 1, {})
  assert_eq(type(cell.options.typing), "table", "typing: true -> table")
end

-- Test: timeout
do
  local cell = build_cell(mock_block("#| timeout: 15.0\necho hi\n"), 1, {})
  assert_eq(cell.options.timeout, 15.0, "timeout: 15.0")
end

-- Test: hold
do
  local cell = build_cell(mock_block("#| hold: 2.0\necho hi\n"), 1, {})
  assert_eq(cell.options.hold, 2.0, "hold: 2.0")
end

-- Test: callouts as list
do
  local cell = build_cell(mock_block("#| callouts: [1, -1]\necho hi\n"), 1, {})
  assert_eq(cell.options.callouts[1], 1, "callouts[1] = 1")
  assert_eq(cell.options.callouts[2], -1, "callouts[2] = -1")
end

-- Test: remove as list
do
  local cell = build_cell(mock_block('#| remove: [1, "pattern"]\necho hi\n'), 1, {})
  assert_eq(cell.options.remove[1], 1, "remove[1] = 1")
  assert_eq(cell.options.remove[2], "pattern", "remove[2] = pattern")
end

-- Test: truncate as list
do
  local cell = build_cell(mock_block('#| truncate: ["3:7", ":5", -1]\necho hi\n'), 1, {})
  assert_eq(cell.options.truncate[1], "3:7", "truncate[1] = 3:7")
  assert_eq(cell.options.truncate[2], ":5", "truncate[2] = :5")
  assert_eq(cell.options.truncate[3], -1, "truncate[3] = -1")
end

-- Test: truncate defaults to empty
do
  local cell = build_cell(mock_block("echo hi\n"), 1, {})
  assert_eq(#cell.options.truncate, 0, "truncate defaults to empty list")
end

-- Test: truncate comma-separated (no brackets)
do
  local cell = build_cell(mock_block('#| truncate: 3:7, :5, -1\necho hi\n'), 1, {})
  assert_eq(cell.options.truncate[1], "3:7", "truncate comma-sep[1] = 3:7")
  assert_eq(cell.options.truncate[2], ":5", "truncate comma-sep[2] = :5")
  assert_eq(cell.options.truncate[3], -1, "truncate comma-sep[3] = -1")
end

-- Test: remove comma-separated (no brackets)
do
  local cell = build_cell(mock_block('#| remove: 1, pattern\necho hi\n'), 1, {})
  assert_eq(cell.options.remove[1], 1, "remove comma-sep[1] = 1")
  assert_eq(cell.options.remove[2], "pattern", "remove comma-sep[2] = pattern")
end

-- Test: callouts comma-separated (no brackets)
do
  local cell = build_cell(mock_block('#| callouts: 1, -1\necho hi\n'), 1, {})
  assert_eq(cell.options.callouts[1], 1, "callouts comma-sep[1] = 1")
  assert_eq(cell.options.callouts[2], -1, "callouts comma-sep[2] = -1")
end

-- Test: truncate single value (no brackets, no comma)
do
  local cell = build_cell(mock_block('#| truncate: 3\necho hi\n'), 1, {})
  assert_eq(#cell.options.truncate, 1, "truncate single has 1 item")
  assert_eq(cell.options.truncate[1], 3, "truncate single[1] = 3")
end

-- Test: highlight
do
  local cell = build_cell(mock_block("#| highlight: python\nprint('hi')\n"), 1, {})
  assert_eq(cell.options.highlight, "python", "highlight: python")
end

-- Test: literal: false (cell-level)
do
  local cell = build_cell(mock_block("#| literal: false\nctrl-c\n"), 1, {})
  assert_eq(cell.options.literal, false, "literal: false")
end

-- Test: delay (cell-level)
do
  local cell = build_cell(mock_block("#| delay: 0.5\necho hi\n"), 1, {})
  assert_eq(cell.options.delay, 0.5, "delay: 0.5")
end

-- Test: style.trailing-spaces (dotted kebab-case)
do
  local cell = build_cell(mock_block("#| style.trailing-spaces: true\necho hi\n"), 1, {})
  assert_eq(cell.options.trailing_spaces, true, "style.trailing-spaces: true")
end

-- Test: style.trailing_spaces (dotted snake_case)
do
  local cell = build_cell(mock_block("#| style.trailing_spaces: true\necho hi\n"), 1, {})
  assert_eq(cell.options.trailing_spaces, true, "style.trailing_spaces snake_case")
end

-- Test: label
do
  local cell = build_cell(mock_block("#| label: my-cell\necho hi\n"), 1, {})
  assert_eq(cell.label, "my-cell", "label: my-cell")
end

-- Test: marker override in chunk
do
  local cell = build_cell(mock_block("#| marker: ##\necho hi ## hold: 1.5\n"), 1, {})
  assert_eq(#cell.line_options, 1, "chunk marker override parses line opts")
  assert_eq(cell.line_options[1].hold, 1.5, "chunk marker: hold = 1.5")
end

-- Test: document-level marker from config
do
  local cell = build_cell(mock_block("echo hi #!! hold: 2.0\n"), 1, { marker = "#!!" })
  assert_eq(#cell.line_options, 1, "doc-level marker parses")
  assert_eq(cell.line_options[1].hold, 2.0, "doc marker: hold = 2.0")
end

-- Test: line_options and source_lines populated
do
  local cell = build_cell(mock_block("echo a\necho b #! delay: 0.3\n"), 1, {})
  assert_eq(#cell.line_options, 1, "one line with options")
  assert_eq(cell.line_options[1].line_index, 1, "line_index = 1 (second line)")
  assert_eq(cell.line_options[1].delay, 0.3, "line delay = 0.3")
  assert_eq(#cell.source_lines, 2, "two source lines")
end

-- Test: style.colorscheme (cell-level, dotted syntax)
do
  local cell = build_cell(mock_block("#| style.colorscheme: nord\necho hi\n"), 1, {})
  assert_eq(cell._colorscheme, "nord", "style.colorscheme: nord")
end

-- Test: colorscheme not set -> nil
do
  local cell = build_cell(mock_block("echo hi\n"), 1, {})
  assert_nil(cell._colorscheme, "no colorscheme -> nil")
end

-- Test: style.html.font-size with format override
do
  local cell = build_cell(mock_block("#| style.font-size: 0.9em\n#| style.html.font-size: 0.7em\necho hi\n"), 1, { _fmt_key = "html" })
  assert_eq(cell.options.font_size, "0.7em", "style.html.font-size overrides base")
end

-- Test: style format override without matching format uses base
do
  local cell = build_cell(mock_block("#| style.font-size: 0.9em\n#| style.html.font-size: 0.7em\necho hi\n"), 1, { _fmt_key = "pdf" })
  assert_eq(cell.options.font_size, "0.9em", "non-matching format uses base style")
end

-- Test: cell-level colorscheme-light/dark
do
  local cell = build_cell(mock_block("#| style.colorscheme-light: github-light\n#| style.colorscheme-dark: github-dark\necho hi\n"), 1, {})
  assert_eq(cell._colorscheme_light, "github-light", "style.colorscheme-light")
  assert_eq(cell._colorscheme_dark, "github-dark", "style.colorscheme-dark")
end

-- Test: eval: false
do
  local cell = build_cell(mock_block("#| eval: false\necho hi\n"), 1, {})
  assert_eq(cell._eval, false, "eval: false")
end

-- Test: eval not set -> true
do
  local cell = build_cell(mock_block("echo hi\n"), 1, {})
  assert_true(cell._eval, "no eval -> true")
end

-- Report
io.stderr:write(string.format("\nbuild_cell: %d passed, %d failed\n", pass_count, fail_count))
if fail_count > 0 then
  os.exit(1)
end

function Pandoc() return nil end
