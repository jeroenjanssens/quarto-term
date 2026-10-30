-- Test cache.lua: successful results are cached, failed results are not
local h = dofile(debug.getinfo(1, "S").source:sub(2):match("(.*[/\\])") .. "helpers.lua")

local cache = dofile(h.project_root() .. "_extensions/term/cache.lua")

local dir = os.tmpname()
os.remove(dir)
os.execute('mkdir -p "' .. dir .. '"')
pandoc.system.get_working_directory = function() return dir end

-- Fake engine: records each call and prints the canned response
local engine = dir .. "/engine.sh"
local f = io.open(engine, "w")
f:write('#!/bin/sh\ncat > /dev/null\necho call >> "' .. dir .. '/calls"\ncat "' .. dir .. '/response.json"\n')
f:close()
os.execute('chmod +x "' .. engine .. '"')

local function respond(json)
  local rf = io.open(dir .. "/response.json", "w")
  rf:write(json)
  rf:close()
end

local function calls()
  local cf = io.open(dir .. "/calls", "r")
  if not cf then return 0 end
  local n = 0
  for _ in cf:lines() do n = n + 1 end
  cf:close()
  return n
end

local function run(input)
  return cache.run(input, { cache = true, engine = engine, version = "test" })
end

-- Test: successful results are served from the cache
respond('[{"id":1,"markup":"ok","error":null}]')
local first = run('{"cells":["success"]}')
h.assert_eq(first[1].markup, "ok", "success: result returned")
run('{"cells":["success"]}')
h.assert_eq(calls(), 1, "success: second run is a cache hit")

-- Test: failed results are not cached, so the next run retries
respond('[{"id":1,"markup":"","error":"timeout waiting for prompt"}]')
local failed = run('{"cells":["failure"]}')
h.assert_eq(failed[1].error, "timeout waiting for prompt", "failure: error returned")
run('{"cells":["failure"]}')
h.assert_eq(calls(), 3, "failure: second run executes again")

-- Test: once the failure is fixed, the result is cached again
respond('[{"id":1,"markup":"fixed","error":null}]')
local fixed = run('{"cells":["failure"]}')
h.assert_eq(fixed[1].markup, "fixed", "fixed: new result returned")
run('{"cells":["failure"]}')
h.assert_eq(calls(), 4, "fixed: second run is a cache hit")

-- Test: an empty error string is not treated as a failure
respond('[{"id":1,"markup":"ok","error":""}]')
run('{"cells":["empty-error"]}')
run('{"cells":["empty-error"]}')
h.assert_eq(calls(), 5, "empty error: result is cached")

-- Test: invalid output (e.g. stray text before the JSON) is not cached
respond('Status: Image is up to date\n[{"id":1,"markup":"ok","error":null}]')
h.assert_nil(run('{"cells":["invalid"]}'), "invalid output: returns nil")
run('{"cells":["invalid"]}')
h.assert_eq(calls(), 7, "invalid output: second run executes again")

os.execute('rm -rf "' .. dir .. '"')

h.finish("cache")
function Pandoc() return nil end
