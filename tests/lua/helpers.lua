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

function M.setup_pandoc_stubs()
  pandoc = {
    List = function(t) return t or {} end,
    system = { get_working_directory = function() return "/tmp" end },
    utils = { stringify = function(v) return tostring(v) end },
  }
end

function M.load_module(name)
  M.setup_pandoc_stubs()
  local root = M.project_root()
  return dofile(root .. "_extensions/term/" .. name .. ".lua")
end

function M.load_cell_module()
  return M.load_module("cell")
end

function M.mock_block(text, classes)
  return { text = text, classes = classes or {"term"} }
end

return M
