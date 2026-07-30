local function find_engine()
  -- Get the directory of this Lua filter file
  local filter_dir = debug.getinfo(1, "S").source:sub(2):match("(.*[/\\])") or "./"

  -- Project root is two levels up from _extensions/term/
  local project_root = filter_dir:match("(.*/?)_extensions/term/") or "./"

  local candidates = {
    project_root .. "target/release/quarto-term",
    project_root .. "target/debug/quarto-term",
    "./target/release/quarto-term",
    "./target/debug/quarto-term",
  }
  for _, path in ipairs(candidates) do
    local f = io.open(path, "r")
    if f then
      f:close()
      return path
    end
  end
  return "quarto-term"
end

local ENGINE = find_engine()

local function escape_pattern(s)
  return s:gsub("([%(%)%.%%%+%-%*%?%[%]%^%$])", "%%%1")
end

local function coerce_value(s)
  s = s:match("^%s*(.-)%s*$")
  if s == "true" then return true end
  if s == "false" then return false end
  local n = tonumber(s)
  if n then return n end
  s = s:gsub('^"(.*)"$', "%1")
  s = s:gsub("^'(.*)'$", "%1")
  return s
end

local function parse_cell_options(text, line_marker)
  local cell_opts = {}
  local code_lines = {}
  local in_options = true

  for line in text:gmatch("([^\n]*)\n?") do
    if in_options and line:match("^#|%s*") then
      local key, value = line:match("^#|%s*(%S+):%s*(.+)$")
      if key then
        local list_match = value:match("^%[(.*)%]$")
        if list_match then
          local items = {}
          for item in list_match:gmatch("[^,]+") do
            table.insert(items, coerce_value(item))
          end
          cell_opts[key] = items
        else
          cell_opts[key] = coerce_value(value)
        end
      end
    else
      in_options = false
      table.insert(code_lines, line)
    end
  end

  if #code_lines > 0 and code_lines[#code_lines] == "" then
    table.remove(code_lines)
  end

  local marker_pat = "%s+" .. escape_pattern(line_marker) .. "%s*(.+)$"
  local parsed_lines = {}
  local line_options = {}

  for idx, raw_line in ipairs(code_lines) do
    local code_part, opts_str = raw_line:match("^(.-)" .. marker_pat)
    if opts_str then
      local opts = { line_index = idx - 1 }
      for k, v in opts_str:gmatch("(%a[%a_]*):%s*([^,]+),?%s*") do
        opts[k] = coerce_value(v)
      end
      table.insert(parsed_lines, code_part)
      table.insert(line_options, opts)
    else
      table.insert(parsed_lines, raw_line)
      table.insert(line_options, { line_index = idx - 1 })
    end
  end

  return cell_opts, table.concat(parsed_lines, "\n"), line_options
end

local function extract_config(meta)
  local config = {
    shell = "zsh",
    shell_args = {},
    prompt = "[\\$#>]\\s*$",
    cols = 80,
    rows = 24,
    ansi = true,
    timeout = 10.0,
    env = {},
    verbose = false,
    format = "html",
  }

  local term_meta = meta and meta["term"]
  if not term_meta then
    return config
  end

  local function meta_str(val)
    if not val then return nil end
    if type(val) == "string" then return val end
    if type(val) == "number" then return tostring(val) end
    if type(val) == "table" then
      local inlines = val
      if val.t == "MetaInlines" then inlines = val end
      -- Single Code element: extract raw text (backtick syntax)
      if #inlines == 1 and inlines[1].t == "Code" then
        return inlines[1].text
      end
      -- Reconstruct text from inlines preserving spaces
      local parts = {}
      for _, el in ipairs(inlines) do
        if el.t == "Str" then
          table.insert(parts, el.text)
        elseif el.t == "Space" then
          table.insert(parts, " ")
        elseif el.t == "Code" then
          table.insert(parts, el.text)
        elseif el.t == "SoftBreak" then
          table.insert(parts, "\n")
        else
          table.insert(parts, pandoc.utils.stringify(el))
        end
      end
      return table.concat(parts)
    end
    return pandoc.utils.stringify(val)
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

  if term_meta["shell"] then
    local shell_val = term_meta["shell"]
    if shell_val.t == "MetaList" then
      config.shell = meta_str(shell_val[1])
      config.shell_args = {}
      for i = 2, #shell_val do
        table.insert(config.shell_args, meta_str(shell_val[i]))
      end
    else
      config.shell = meta_str(shell_val)
    end
  end

  if term_meta["shell_args"] then
    local args_val = term_meta["shell_args"]
    config.shell_args = {}
    if type(args_val) == "table" then
      for i = 1, #args_val do
        table.insert(config.shell_args, meta_str(args_val[i]))
      end
    end
  end

  if term_meta["prompt"] then
    -- Special handling: try to preserve regex syntax that Pandoc may mangle.
    -- If the value is a MetaInlines with Code elements, extract from code.
    local prompt_val = term_meta["prompt"]
    if prompt_val.t == "MetaInlines" and #prompt_val == 1 and prompt_val[1].t == "Code" then
      config.prompt = prompt_val[1].text
    else
      config.prompt = meta_str(prompt_val)
    end
  end
  if term_meta["cols"] then config.cols = meta_num(term_meta["cols"]) or config.cols end
  if term_meta["rows"] then config.rows = meta_num(term_meta["rows"]) or config.rows end
  if term_meta["ansi"] ~= nil then config.ansi = meta_bool(term_meta["ansi"]) end
  if term_meta["timeout"] then config.timeout = meta_num(term_meta["timeout"]) or config.timeout end
  if term_meta["verbose"] ~= nil then config.verbose = meta_bool(term_meta["verbose"]) end
  if term_meta["spacing"] ~= nil then config.spacing = meta_bool(term_meta["spacing"]) end
  if term_meta["record"] then config.record = meta_str(term_meta["record"]) end

  if term_meta["env"] then
    local env_val = term_meta["env"]
    if type(env_val) == "table" then
      for k, v in pairs(env_val) do
        if type(k) == "string" then
          config.env[k] = meta_str(v)
        end
      end
    end
  end

  -- Pandoc strips trailing spaces from YAML values; restore for prompt vars
  for _, key in ipairs({"PS1", "PS2", "PROMPT"}) do
    local val = config.env[key]
    if val and not val:match("%s$") then
      config.env[key] = val .. " "
    end
  end

  return config
end

local function is_term_block(block)
  if block.t ~= "CodeBlock" then
    return false
  end
  for _, cls in ipairs(block.classes) do
    if cls == "term" or cls == "{term}" then
      return true
    end
  end
  return false
end

local function build_cell(block, cell_id, config)
  local line_marker = "#!"
  local cell_opts, code, line_options = parse_cell_options(block.text, line_marker)

  local options = {
    echo = "terminal",
    output = true,
    fullscreen = false,
    scroll = true,
    keep_last_prompt = false,
    callouts = pandoc.List({}),
    remove = pandoc.List({}),
    highlight = "bash",
  }

  if cell_opts["echo"] ~= nil then options.echo = cell_opts["echo"] end
  if cell_opts["output"] ~= nil then options.output = cell_opts["output"] end
  if cell_opts["fullscreen"] ~= nil then options.fullscreen = cell_opts["fullscreen"] end
  if cell_opts["scroll"] ~= nil then options.scroll = cell_opts["scroll"] end
  if cell_opts["keep_last_prompt"] ~= nil then options.keep_last_prompt = cell_opts["keep_last_prompt"] end
  if cell_opts["ansi"] ~= nil then options.ansi = cell_opts["ansi"] end
  if cell_opts["spacing"] ~= nil then options.spacing = cell_opts["spacing"] end
  if cell_opts["callouts"] ~= nil then options.callouts = cell_opts["callouts"] end
  if cell_opts["remove"] ~= nil then options.remove = cell_opts["remove"] end
  if cell_opts["highlight"] ~= nil then options.highlight = cell_opts["highlight"] end

  return {
    id = cell_id,
    code = code,
    options = options,
    line_options = line_options,
  }
end

function Pandoc(doc)
  local config = extract_config(doc.meta)

  if quarto and quarto.doc and quarto.doc.is_format then
    if quarto.doc.is_format("pdf") or quarto.doc.is_format("latex") then
      config.format = "latex"
    elseif quarto.doc.is_format("markdown") then
      config.format = "markdown"
    end
  end

  local term_positions = {}
  local cells = {}

  for i, block in ipairs(doc.blocks) do
    if is_term_block(block) then
      local cell_id = #cells + 1
      local cell = build_cell(block, cell_id, config)
      table.insert(cells, cell)
      table.insert(term_positions, { block_i = i, cell_i = cell_id })
    end
  end

  if #cells == 0 then
    return nil
  end

  -- Ensure arrays are encoded as JSON arrays, not objects
  if #config.shell_args == 0 then
    config.shell_args = pandoc.List({})
  end

  local request = {
    config = config,
    cells = cells,
  }

  local input_json = pandoc.json.encode(request)
  local ok, output = pcall(pandoc.pipe, ENGINE, {}, input_json)

  if not ok then
    io.stderr:write("quarto-term: engine error: " .. tostring(output) .. "\n")
    return nil
  end

  local results = pandoc.json.decode(output)

  for _, pos in ipairs(term_positions) do
    local result = results[pos.cell_i]
    if result then
      if result.error and result.error ~= pandoc.json.null and result.error ~= "" then
        io.stderr:write("quarto-term: cell " .. pos.cell_i .. " error: " .. tostring(result.error) .. "\n")
      end
      local html = result.html
      if type(html) == "string" and html ~= "" then
        doc.blocks[pos.block_i] = pandoc.RawBlock("html", html)
      else
        doc.blocks[pos.block_i] = pandoc.Null()
      end
    end
  end

  if quarto and quarto.doc and quarto.doc.add_html_dependency then
    quarto.doc.add_html_dependency({
      name = "quarto-term",
      version = "0.2.0",
      stylesheets = { "term.css" },
    })
  end

  return doc
end
