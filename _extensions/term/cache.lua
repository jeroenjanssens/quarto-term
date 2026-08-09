local M = {}

function M.run(input_json, opts)
  local use_cache = opts.cache ~= false
  local engine = opts.engine
  local verbose = opts.verbose
  local version = opts.version

  if use_cache then
    local cache_key = pandoc.utils.sha1(version .. "\n" .. input_json)
    local cache_dir = pandoc.system.get_working_directory() .. "/.quarto-term-cache"
    local cache_file = cache_dir .. "/" .. cache_key .. ".json"
    local cf = io.open(cache_file, "r")
    if cf then
      local cached = cf:read("*a")
      cf:close()
      if verbose then
        io.stderr:write("quarto-term: cache hit (" .. cache_key:sub(1, 8) .. ")\n")
      end
      return pandoc.json.decode(cached)
    else
      local ok, output = pcall(pandoc.pipe, engine, {}, input_json)
      if not ok then
        io.stderr:write("quarto-term: engine error: " .. tostring(output) .. "\n")
        return nil
      end
      os.execute('mkdir -p "' .. cache_dir .. '"')
      local wf = io.open(cache_file, "w")
      if wf then
        wf:write(output)
        wf:close()
      end
      if verbose then
        io.stderr:write("quarto-term: cache miss, stored (" .. cache_key:sub(1, 8) .. ")\n")
      end
      return pandoc.json.decode(output)
    end
  else
    local ok, output = pcall(pandoc.pipe, engine, {}, input_json)
    if not ok then
      io.stderr:write("quarto-term: engine error: " .. tostring(output) .. "\n")
      return nil
    end
    return pandoc.json.decode(output)
  end
end

return M
