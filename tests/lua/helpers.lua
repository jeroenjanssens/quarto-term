local M = {}

M.pass_count = 0
M.fail_count = 0

function M.assert_eq(actual, expected, msg)
  if actual == expected then
    M.pass_count = M.pass_count + 1
  else
    M.fail_count = M.fail_count + 1
    io.stderr:write(string.format("FAIL: %s\n  expected: %s (%s)\n  actual:   %s (%s)\n",
      msg, tostring(expected), type(expected), tostring(actual), type(actual)))
  end
end

function M.assert_true(val, msg)
  M.assert_eq(val, true, msg)
end

function M.assert_nil(val, msg)
  if val == nil then
    M.pass_count = M.pass_count + 1
  else
    M.fail_count = M.fail_count + 1
    io.stderr:write(string.format("FAIL: %s\n  expected nil, got: %s (%s)\n",
      msg, tostring(val), type(val)))
  end
end

function M.assert_not_nil(val, msg)
  if val ~= nil then
    M.pass_count = M.pass_count + 1
  else
    M.fail_count = M.fail_count + 1
    io.stderr:write(string.format("FAIL: %s\n  expected non-nil value\n", msg))
  end
end

function M.finish(test_name)
  print(string.format("%s: %d passed, %d failed", test_name, M.pass_count, M.fail_count))
  if M.fail_count > 0 then os.exit(1) end
end

function M.project_root()
  local filter_dir = debug.getinfo(2, "S").source:sub(2):match("(.*[/\\])") or "./"
  return filter_dir:match("(.*/?)tests/lua/") or "./"
end

function M.read_term_source()
  local root = M.project_root()
  local term_lua = root .. "_extensions/term/term.lua"
  local f = io.open(term_lua, "r")
  if not f then
    io.stderr:write("ERROR: cannot open " .. term_lua .. "\n")
    os.exit(1)
  end
  local source = f:read("*a")
  f:close()
  return source
end

function M.load_term_functions(names, opts)
  opts = opts or {}
  local source = M.read_term_source()
  local prelude = opts.prelude or 'local pandoc = { List = function(t) return t or {} end }'

  local parts = { prelude }
  for _, name in ipairs(names) do
    local pattern = "(local function " .. name .. ".-\nend)"
    local src = source:match(pattern)
    if not src then
      io.stderr:write("ERROR: cannot extract function '" .. name .. "' from term.lua\n")
      os.exit(1)
    end
    table.insert(parts, src)
  end

  local last_name = names[#names]
  table.insert(parts, "return " .. last_name)
  local chunk = table.concat(parts, "\n")
  local fn = load(chunk)
  if not fn then
    io.stderr:write("ERROR: failed to load extracted chunk\n")
    os.exit(1)
  end
  return fn()
end

function M.load_multiple_functions(names, opts)
  opts = opts or {}
  local source = M.read_term_source()
  local prelude = opts.prelude or 'local pandoc = { List = function(t) return t or {} end }'

  local parts = { prelude }
  for _, name in ipairs(names) do
    local pattern = "(local function " .. name .. ".-\nend)"
    local src = source:match(pattern)
    if not src then
      io.stderr:write("ERROR: cannot extract function '" .. name .. "' from term.lua\n")
      os.exit(1)
    end
    table.insert(parts, src)
  end

  local returns = {}
  for _, name in ipairs(names) do
    table.insert(returns, name .. " = " .. name)
  end
  table.insert(parts, "return { " .. table.concat(returns, ", ") .. " }")
  local chunk = table.concat(parts, "\n")
  local fn = load(chunk)
  if not fn then
    io.stderr:write("ERROR: failed to load extracted chunk\n")
    os.exit(1)
  end
  return fn()
end

function M.mock_block(text, classes)
  return { text = text, classes = classes or {"term"} }
end

return M
