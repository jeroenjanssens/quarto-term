-- Test coerce_value function from term.lua
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

-- Load coerce_value from term.lua
local filter_dir = debug.getinfo(1, "S").source:sub(2):match("(.*[/\\])") or "./"
local project_root = filter_dir:match("(.*/?)tests/lua/") or "./"
local term_lua = project_root .. "_extensions/term/term.lua"

-- Extract just the coerce_value function by loading the source
local f = io.open(term_lua, "r")
if not f then
  io.stderr:write("ERROR: cannot open " .. term_lua .. "\n")
  os.exit(1)
end
local source = f:read("*a")
f:close()

-- Extract the function body
local func_source = source:match("(local function coerce_value.-\nend)")
if not func_source then
  io.stderr:write("ERROR: cannot find coerce_value function\n")
  os.exit(1)
end

-- Compile and execute to get the function
local coerce_value = load(func_source .. "\nreturn coerce_value")()

-- Tests
assert_eq(coerce_value("true"), true, "string 'true' -> boolean true")
assert_eq(coerce_value("false"), false, "string 'false' -> boolean false")
assert_eq(coerce_value("42"), 42, "string '42' -> number 42")
assert_eq(coerce_value("3.14"), 3.14, "string '3.14' -> number 3.14")
assert_eq(coerce_value("0"), 0, "string '0' -> number 0")
assert_eq(coerce_value("-1"), -1, "string '-1' -> number -1")
assert_eq(coerce_value('"hello"'), "hello", "double-quoted string")
assert_eq(coerce_value("'world'"), "world", "single-quoted string")
assert_eq(coerce_value("  trimmed  "), "trimmed", "whitespace trimming")
assert_eq(coerce_value("bash"), "bash", "plain string passthrough")
assert_eq(coerce_value("  true  "), true, "trimmed boolean")
assert_eq(coerce_value("source"), "source", "non-numeric/non-bool string")

-- Report
io.stderr:write(string.format("\ncoerce_value: %d passed, %d failed\n", pass_count, fail_count))
if fail_count > 0 then
  os.exit(1)
end

function Pandoc() return nil end
