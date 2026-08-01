-- Test extract_config function from term.lua
-- This test uses pandoc's Lua filter API to construct meta tables
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

local function assert_not_nil(val, msg)
  if val ~= nil then
    pass_count = pass_count + 1
  else
    fail_count = fail_count + 1
    io.stderr:write(string.format("FAIL: %s\n  expected non-nil, got nil\n", msg))
  end
end

-- Load term.lua functions by extracting them
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

-- Extract all needed functions
local resolve_init_src = source:match("(local function resolve_init_path.-\nend)")
local extract_config_src = source:match("(local function extract_config.-\n  return config\nend)")

-- Build a loadable chunk with stubs
local chunk = [[
local function meta_str(val)
  if not val then return nil end
  if type(val) == "string" then return val end
  if type(val) == "number" then return tostring(val) end
  if type(val) == "table" then
    if val.t == "MetaInlines" or val[1] then
      local parts = {}
      for _, el in ipairs(val) do
        if type(el) == "string" then
          table.insert(parts, el)
        elseif type(el) == "table" and el.t == "Str" then
          table.insert(parts, el.text)
        end
      end
      return table.concat(parts)
    end
  end
  return tostring(val)
end
local function meta_bool(val)
  if val == nil then return nil end
  if type(val) == "boolean" then return val end
  local s = meta_str(val)
  if s == "true" then return true end
  if s == "false" then return false end
  return nil
end
local function meta_num(val)
  if not val then return nil end
  if type(val) == "number" then return val end
  return tonumber(meta_str(val))
end
]] .. resolve_init_src .. "\n" .. extract_config_src .. "\nreturn extract_config"

local extract_config = load(chunk)()

-- Helper to make simple meta values
local function str_val(s) return s end

-- Test: nil meta returns defaults
do
  local config = extract_config(nil)
  assert_eq(config.shell, "zsh", "default shell is zsh")
  assert_eq(config.prompt, "$", "default prompt is $")
  assert_eq(config.cols, 80, "default cols = 80")
  assert_eq(config.rows, 24, "default rows = 24")
  assert_eq(config.ansi, true, "default ansi = true")
  assert_eq(config.timeout, 10.0, "default timeout = 10")
end

-- Test: empty extensions.term returns defaults
do
  local meta = { extensions = {} }
  local config = extract_config(meta)
  assert_eq(config.shell, "zsh", "no term key -> defaults")
end

-- Test: shell: bash sets shell_args to --norc --noprofile
do
  local meta = { extensions = { term = { shell = "bash" } } }
  local config = extract_config(meta)
  assert_eq(config.shell, "bash", "shell = bash")
  assert_eq(config.shell_args[1], "--norc", "bash default arg 1")
  assert_eq(config.shell_args[2], "--noprofile", "bash default arg 2")
end

-- Test: shell: zsh sets shell_args to --no-rcs
do
  local meta = { extensions = { term = { shell = "zsh" } } }
  local config = extract_config(meta)
  assert_eq(config.shell_args[1], "--no-rcs", "zsh default arg")
end

-- Test: shell: fish sets shell_args to --no-config
do
  local meta = { extensions = { term = { shell = "fish" } } }
  local config = extract_config(meta)
  assert_eq(config.shell_args[1], "--no-config", "fish default arg")
end

-- Test: prompt sets PS1
do
  local meta = { extensions = { term = { prompt = ">>>" } } }
  local config = extract_config(meta)
  assert_eq(config.prompt, ">>>", "prompt = >>>")
  assert_eq(config.env["PS1"], ">>> ", "PS1 = '>>> ' (with trailing space)")
end

-- Test: PS2 defaults to "> "
do
  local meta = { extensions = { term = {} } }
  local config = extract_config(meta)
  assert_eq(config.env["PS2"], "> ", "default PS2 = '> '")
  assert_eq(config.ps2, "> ", "config.ps2 = '> '")
end

-- Test: typing: "false" (as pandoc would pass it from YAML)
do
  local meta = { extensions = { term = { typing = "false" } } }
  local config = extract_config(meta)
  assert_eq(config.typing, false, "typing: 'false' -> false")
end

-- Test: record as list
do
  local meta = { extensions = { term = { record = { "out.cast", "out.termshow" } } } }
  local config = extract_config(meta)
  assert_eq(config.record[1], "out.cast", "record[1]")
  assert_eq(config.record[2], "out.termshow", "record[2]")
end

-- Test: record as single string
do
  local meta = { extensions = { term = { record = "out.cast" } } }
  local config = extract_config(meta)
  assert_eq(config.record[1], "out.cast", "record as single string")
end

-- Test: env variables
do
  local meta = { extensions = { term = { env = { MY_VAR = "hello" } } } }
  local config = extract_config(meta)
  assert_eq(config.env["MY_VAR"], "hello", "env.MY_VAR = hello")
end

-- Test: trailing-spaces (kebab-case)
do
  local meta = { extensions = { term = { ["trailing-spaces"] = true } } }
  local config = extract_config(meta)
  assert_eq(config.trailing_spaces, true, "trailing-spaces: true")
end

-- Test: spacing
do
  local meta = { extensions = { term = { spacing = true } } }
  local config = extract_config(meta)
  assert_eq(config.spacing, true, "spacing: true")
end

-- Test: verbose
do
  local meta = { extensions = { term = { verbose = true } } }
  local config = extract_config(meta)
  assert_eq(config.verbose, true, "verbose: true")
end

-- Test: cols and rows
do
  local meta = { extensions = { term = { cols = 120, rows = 40 } } }
  local config = extract_config(meta)
  assert_eq(config.cols, 120, "cols = 120")
  assert_eq(config.rows, 40, "rows = 40")
end

-- Test: timeout
do
  local meta = { extensions = { term = { timeout = 30.0 } } }
  local config = extract_config(meta)
  assert_eq(config.timeout, 30.0, "timeout = 30")
end

-- Test: ansi: false
do
  local meta = { extensions = { term = { ansi = false } } }
  local config = extract_config(meta)
  assert_eq(config.ansi, false, "ansi: false")
end

-- Test: custom shell-args (kebab-case)
do
  local meta = { extensions = { term = { shell = "bash", ["shell-args"] = { "--login" } } } }
  local config = extract_config(meta)
  assert_eq(config.shell_args[1], "--login", "custom shell-args")
end

-- Test: prompt-regex
do
  local meta = { extensions = { term = { ["prompt-regex"] = "\\$\\s*$" } } }
  local config = extract_config(meta)
  assert_eq(config.prompt_regex, "\\$\\s*$", "prompt-regex")
end

-- Test: fontsize as string
do
  local meta = { extensions = { term = { fontsize = "0.8em" } } }
  local config = extract_config(meta)
  assert_eq(config.fontsize, "0.8em", "fontsize = 0.8em")
end

-- Test: fontsize as per-format map
do
  local meta = { extensions = { term = { fontsize = { html = "0.8em", pdf = "0.7em" } } } }
  local config = extract_config(meta)
  assert_not_nil(config._fontsize_map, "fontsize map exists")
  assert_eq(config._fontsize_map["html"], "0.8em", "fontsize_map.html")
  assert_eq(config._fontsize_map["pdf"], "0.7em", "fontsize_map.pdf")
end

-- Test: theme as single string
do
  local meta = { extensions = { term = { theme = "nord" } } }
  local config = extract_config(meta)
  assert_eq(config.theme, "nord", "theme = nord")
end

-- Test: theme as light/dark map
do
  local meta = { extensions = { term = { theme = { light = "solarized-light", dark = "solarized-dark" } } } }
  local config = extract_config(meta)
  assert_eq(config.theme_light, "solarized-light", "theme.light")
  assert_eq(config.theme_dark, "solarized-dark", "theme.dark")
end

-- Test: theme-bg and theme-fg
do
  local meta = { extensions = { term = { ["theme-bg"] = "#1a1b26", ["theme-fg"] = "#c0caf5" } } }
  local config = extract_config(meta)
  assert_eq(config.theme_bg, "#1a1b26", "theme-bg")
  assert_eq(config.theme_fg, "#c0caf5", "theme-fg")
end

-- Test: marker option
do
  local meta = { extensions = { term = { marker = "#!!" } } }
  local config = extract_config(meta)
  assert_eq(config.marker, "#!!", "marker = #!!")
end

-- Test: init as inline command
do
  local meta = { extensions = { term = { init = "setopt INTERACTIVE_COMMENTS" } } }
  local config = extract_config(meta)
  assert_eq(config.init[1], "setopt INTERACTIVE_COMMENTS", "init inline command")
end

-- Test: init as list with inline commands
do
  local meta = { extensions = { term = { init = { "setopt INTERACTIVE_COMMENTS", "alias ll='ls -la'" } } } }
  local config = extract_config(meta)
  assert_eq(config.init[1], "setopt INTERACTIVE_COMMENTS", "init list inline[1]")
  assert_eq(config.init[2], "alias ll='ls -la'", "init list inline[2]")
end

-- Test: init multi-line block splits into separate commands
do
  local meta = { extensions = { term = { init = "setopt INTERACTIVE_COMMENTS\nalias ll='ls -la'" } } }
  local config = extract_config(meta)
  assert_eq(config.init[1], "setopt INTERACTIVE_COMMENTS", "init multiline[1]")
  assert_eq(config.init[2], "alias ll='ls -la'", "init multiline[2]")
end

-- Test: init with existing file gets source prefix
do
  -- Use a file we know exists
  local meta = { extensions = { term = { init = "/dev/null" } } }
  local config = extract_config(meta)
  assert_eq(config.init[1], "source /dev/null", "init existing file gets source prefix")
end

-- Test: trailing_spaces (snake_case compat)
do
  local meta = { extensions = { term = { ["trailing_spaces"] = true } } }
  local config = extract_config(meta)
  assert_eq(config.trailing_spaces, true, "trailing_spaces snake_case")
end

-- Report
io.stderr:write(string.format("\nextract_config: %d passed, %d failed\n", pass_count, fail_count))
if fail_count > 0 then
  os.exit(1)
end

function Pandoc() return nil end
