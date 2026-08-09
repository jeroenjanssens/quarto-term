-- Test coerce_value function from term.lua
local h = dofile(debug.getinfo(1, "S").source:sub(2):match("(.*[/\\])") .. "helpers.lua")

local coerce_value = h.load_term_functions({"coerce_value"}, { prelude = "" })

-- Tests
h.assert_eq(coerce_value("true"), true, "string 'true' -> boolean true")
h.assert_eq(coerce_value("false"), false, "string 'false' -> boolean false")
h.assert_eq(coerce_value("42"), 42, "string '42' -> number 42")
h.assert_eq(coerce_value("3.14"), 3.14, "string '3.14' -> number 3.14")
h.assert_eq(coerce_value("0"), 0, "string '0' -> number 0")
h.assert_eq(coerce_value("-1"), -1, "string '-1' -> number -1")
h.assert_eq(coerce_value('"hello"'), "hello", "double-quoted string")
h.assert_eq(coerce_value("'world'"), "world", "single-quoted string")
h.assert_eq(coerce_value("  trimmed  "), "trimmed", "whitespace trimming")
h.assert_eq(coerce_value("bash"), "bash", "plain string passthrough")
h.assert_eq(coerce_value("  true  "), true, "trimmed boolean")
h.assert_eq(coerce_value("source"), "source", "non-numeric/non-bool string")

h.finish("coerce_value")
function Pandoc() return nil end
