local FILTER_DIR = debug.getinfo(1, "S").source:sub(2):match("(.*[/\\])") or "./"
local VERSION = "0.4.4"

local cell_mod = dofile(FILTER_DIR .. "cell.lua")
local chrome = dofile(FILTER_DIR .. "chrome.lua")
local engine = dofile(FILTER_DIR .. "engine.lua")
local theme = dofile(FILTER_DIR .. "theme.lua")
theme.init(FILTER_DIR)

local ENGINE = engine.find(FILTER_DIR)


local escape_pattern = cell_mod.escape_pattern
local coerce_value = cell_mod.coerce_value
local as_list = cell_mod.as_list

local parse_cell_options = cell_mod.parse_cell_options

local function resolve_init_path(path)
  if path:sub(1, 1) == "/" then
    local f = io.open(path, "r")
    if f then f:close(); return path end
    return nil
  end
  -- Check current working directory first
  local local_file = io.open(path, "r")
  if local_file then
    local_file:close()
    return path
  end
  local search_dirs = {
    "/opt/homebrew/share",
    "/usr/local/share",
    "/usr/share",
    "/home/linuxbrew/.linuxbrew/share",
  }
  for _, dir in ipairs(search_dirs) do
    local candidate = dir .. "/" .. path .. "/" .. path .. ".zsh"
    local f = io.open(candidate, "r")
    if f then
      f:close()
      return candidate
    end
  end
  return nil
end

local SHELL_PRESETS = {
  python = { shell = "python3", shell_args = {"-q"}, prompt = ">>>", ps2 = "..." },
  node = { shell = "node", shell_args = {}, prompt = ">", ps2 = "..." },
  r = { shell = "R", shell_args = {"--no-save", "--no-restore", "--quiet"}, prompt = ">", ps2 = "+" },
  ruby = { shell = "irb", shell_args = {"--noecho", "--nomultiline"}, prompt = "irb>", ps2 = "irb*" },
  sqlite = { shell = "sqlite3", shell_args = {}, prompt = "sqlite>", ps2 = "   ...>" },
  duckdb = { shell = "duckdb", shell_args = {}, prompt = "D", ps2 = ">" },
  fish = { shell = "fish", shell_args = {"--no-config"}, prompt = ">", ps2 = "" },
  nushell = { shell = "nu", shell_args = {"--no-config-file"}, prompt = ">", ps2 = ":::" },
  sh = { shell = "sh", shell_args = {}, prompt = "$", ps2 = ">" },
}

local function extract_config(meta)
  local config = {
    shell = "zsh",
    shell_args = {},
    prompt = "$",
    cols = 80,
    rows = 24,
    ansi = true,
    timeout = 10.0,
    env = {},
    verbose = false,
    format = "html",
  }

  local term_meta = meta and meta["extensions"] and meta["extensions"]["term"]
  if not term_meta then
    return config
  end

  -- Apply shell preset as defaults (user-specified values override)
  if term_meta["profile"] then
    local profile_name = type(term_meta["profile"]) == "string"
      and term_meta["profile"] or pandoc.utils.stringify(term_meta["profile"])
    local preset = SHELL_PRESETS[profile_name]
    if preset then
      config.shell = preset.shell
      config.shell_args = preset.shell_args
      config.prompt = preset.prompt
      if preset.ps2 then config._ps2_explicit = preset.ps2 end
    end
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
          -- Reverse pandoc smart typography on config values:
          -- em-dash (U+2014) back to ---, en-dash (U+2013) back to --
          local text = el.text:gsub("\xe2\x80\x94", "---"):gsub("\xe2\x80\x93", "--")
          table.insert(parts, text)
        elseif el.t == "Space" then
          table.insert(parts, " ")
        elseif el.t == "Code" then
          table.insert(parts, el.text)
        elseif el.t == "SoftBreak" then
          table.insert(parts, "\n")
        elseif el.t == "Quoted" then
          local q = el.quotetype == "SingleQuote" and "'" or '"'
          table.insert(parts, q .. pandoc.utils.stringify(el.content) .. q)
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

  -- Accept both kebab-case (preferred) and snake_case (compat) for shell-args
  local shell_args_val = term_meta["shell-args"] or term_meta["shell_args"]
  if shell_args_val then
    config.shell_args = {}
    if type(shell_args_val) == "table" then
      for i = 1, #shell_args_val do
        table.insert(config.shell_args, meta_str(shell_args_val[i]))
      end
    end
  else
    -- Default: suppress rc files for reproducibility
    local shell_base = config.shell:match("([^/]+)$") or config.shell
    if shell_base == "zsh" then
      config.shell_args = {"--no-rcs"}
    elseif shell_base == "bash" then
      config.shell_args = {"--norc", "--noprofile"}
    elseif shell_base == "fish" then
      config.shell_args = {"--no-config"}
    end
  end

  local prompt_val = term_meta["prompt"] or term_meta["ps1"]
  if prompt_val then
    config.prompt = meta_str(prompt_val)
  end

  -- prompt-regex / ps1-regex: raw regex override for prompt matching
  local prompt_regex_val = term_meta["prompt-regex"] or term_meta["prompt_regex"]
    or term_meta["ps1-regex"] or term_meta["ps1_regex"]
  if prompt_regex_val then
    config.prompt_regex = meta_str(prompt_regex_val)
  end
  local ps2_val = term_meta["ps2"]
  if ps2_val then
    config._ps2_explicit = meta_str(ps2_val)
  end
  local ps2_regex_val = term_meta["ps2-regex"] or term_meta["ps2_regex"]
  if ps2_regex_val then
    config.ps2_regex = meta_str(ps2_regex_val)
  end
  if term_meta["timeout"] then config.timeout = meta_num(term_meta["timeout"]) or config.timeout end
  if term_meta["init"] ~= nil then
    local init_val = term_meta["init"]
    if init_val == false or (type(init_val) == "userdata" and tostring(init_val) == "false") then
      config.init = {}
    else
      local init_list = {}
      if type(init_val) == "table" and init_val.t == nil
          and #init_val > 0 and type(init_val[1]) ~= "userdata" then
        for i = 1, #init_val do
          table.insert(init_list, meta_str(init_val[i]))
        end
      else
        table.insert(init_list, meta_str(init_val))
      end
      config.init = {}
      for _, entry in ipairs(init_list) do
        local resolved = resolve_init_path(entry)
        if resolved then
          table.insert(config.init, "source " .. resolved)
        else
          for line in entry:gmatch("[^\n]+") do
            local trimmed = line:match("^%s*(.-)%s*$")
            if trimmed and trimmed ~= "" then
              table.insert(config.init, trimmed)
            end
          end
        end
      end
    end
  end
  if term_meta["cache"] ~= nil then config._cache = meta_bool(term_meta["cache"]) end
  if term_meta["verbose"] ~= nil then config.verbose = meta_bool(term_meta["verbose"]) end
  if term_meta["strip-auto-indent"] ~= nil then config.strip_auto_indent = meta_bool(term_meta["strip-auto-indent"]) end
  if term_meta["spacing"] ~= nil then config.spacing = meta_bool(term_meta["spacing"]) end
  if term_meta["delay"] ~= nil then config.delay = meta_num(term_meta["delay"]) end
  if term_meta["hold"] ~= nil then config.hold = meta_num(term_meta["hold"]) end
  if term_meta["echo"] ~= nil then
    local b = meta_bool(term_meta["echo"])
    if b ~= nil then config.echo = b
    else config.echo = meta_str(term_meta["echo"]) end
  end
  local klp_val = term_meta["keep-last-prompt"] or term_meta["keep_last_prompt"]
  if klp_val ~= nil then config.keep_last_prompt = meta_bool(klp_val) end
  if term_meta["highlight"] ~= nil then
    local b = meta_bool(term_meta["highlight"])
    if b ~= nil then config.highlight = b
    else config.highlight = meta_str(term_meta["highlight"]) end
  end

  -- marker option (used only in Lua, not sent to Rust)
  if term_meta["marker"] then
    config.marker = meta_str(term_meta["marker"])
  end

  -- Style block
  local FORMAT_KEYS = { html = true, pdf = true, revealjs = true, epub = true, markdown = true, latex = true, typst = true }

  local function extract_style(style_meta)
    if not style_meta or type(style_meta) ~= "table" then return {} end
    local s = {}
    if style_meta["colorscheme"] then s.colorscheme = meta_str(style_meta["colorscheme"]) end
    local csl = style_meta["colorscheme-light"] or style_meta["colorscheme_light"]
    if csl then s.colorscheme_light = meta_str(csl) end
    local csd = style_meta["colorscheme-dark"] or style_meta["colorscheme_dark"]
    if csd then s.colorscheme_dark = meta_str(csd) end
    local ff = style_meta["font-family"] or style_meta["font_family"]
    if ff then s.font_family = meta_str(ff) end
    local fs = style_meta["font-size"] or style_meta["font_size"]
    if fs then s.font_size = meta_str(fs) end
    local lh = style_meta["line-height"] or style_meta["line_height"]
    if lh then s.line_height = meta_str(lh) end
    local ts = style_meta["trailing-spaces"] or style_meta["trailing_spaces"]
    if ts ~= nil then s.trailing_spaces = meta_bool(ts) end
    if style_meta["ansi"] ~= nil then s.ansi = meta_bool(style_meta["ansi"]) end
    if style_meta["spacing"] ~= nil then s.spacing = meta_bool(style_meta["spacing"]) end
    if style_meta["cols"] then s.cols = meta_num(style_meta["cols"]) end
    if style_meta["rows"] then s.rows = meta_num(style_meta["rows"]) end
    if style_meta["chrome"] ~= nil then
      local cv = style_meta["chrome"]
      if type(cv) == "boolean" or (type(cv) == "userdata" and (tostring(cv) == "true" or tostring(cv) == "false")) then
        s.chrome = meta_bool(cv)
      elseif type(cv) == "table" and (cv["os"] ~= nil or cv["color"] ~= nil
          or cv["title"] ~= nil or cv["shadow"] ~= nil) then
        s.chrome = {
          os     = cv["os"] and meta_str(cv["os"]) or nil,
          color  = cv["color"] and meta_str(cv["color"]) or nil,
          title  = cv["title"] and meta_str(cv["title"]) or nil,
          shadow = cv["shadow"] ~= nil and meta_bool(cv["shadow"]) or nil,
        }
      else
        s.chrome = meta_str(cv)
      end
    end
    -- Collect format-specific overrides
    s._format_overrides = {}
    for k, v in pairs(style_meta) do
      if type(k) == "string" and FORMAT_KEYS[k] and type(v) == "table" then
        s._format_overrides[k] = v
      end
    end
    if next(s._format_overrides) == nil then s._format_overrides = nil end
    return s
  end

  if term_meta["style"] then
    local style = extract_style(term_meta["style"])
    if style.colorscheme then config._colorscheme = style.colorscheme end
    if style.colorscheme_light then config._colorscheme_light = style.colorscheme_light end
    if style.colorscheme_dark then config._colorscheme_dark = style.colorscheme_dark end
    if style.font_family then config.font_family = style.font_family end
    if style.font_size then config.font_size = style.font_size end
    if style.line_height then config.line_height = style.line_height end
    if style.trailing_spaces ~= nil then config.trailing_spaces = style.trailing_spaces end
    if style.ansi ~= nil then config.ansi = style.ansi end
    if style.spacing ~= nil then config.spacing = style.spacing end
    if style.cols then config.cols = style.cols end
    if style.rows then config.rows = style.rows end
    if style.chrome ~= nil then config._chrome = style.chrome end
    if style._format_overrides then
      -- Pre-extract each format override so we don't need the function later
      config._style_overrides = {}
      for fmt, raw in pairs(style._format_overrides) do
        config._style_overrides[fmt] = extract_style(raw)
      end
    end
  end
  if term_meta["typing"] then
    local typing_val = term_meta["typing"]
    if meta_bool(typing_val) == false then
      config.typing = false
    elseif type(typing_val) == "table" and typing_val.t == nil then
      config.typing = {}
      if typing_val["speed"] then config.typing.speed = meta_num(typing_val["speed"]) end
      -- Accept both kebab-case and snake_case for error-rate
      local error_rate_val = typing_val["error-rate"] or typing_val["error_rate"]
      if error_rate_val then config.typing.error_rate = meta_num(error_rate_val) end
    end
  end
  if term_meta["record"] then
    local record_val = term_meta["record"]
    if type(record_val) == "table" and record_val.t == nil then
      config.record = {}
      for i = 1, #record_val do
        table.insert(config.record, meta_str(record_val[i]))
      end
    else
      config.record = { meta_str(record_val) }
    end
  end

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

  -- Auto-set PS1 from prompt if not explicitly provided in env
  if not config.env["PS1"] then
    config.env["PS1"] = config.prompt .. " "
  end

  -- Set a known PS2 so we can detect continuation prompts
  if config._ps2_explicit then
    config.ps2 = config._ps2_explicit
    if not config.env["PS2"] then
      config.env["PS2"] = config._ps2_explicit
    end
  else
    if not config.env["PS2"] then
      config.env["PS2"] = "> "
    end
    config.ps2 = config.env["PS2"]
  end
  config._ps2_explicit = nil

  -- Pandoc strips trailing spaces from YAML values; restore for prompt vars
  for _, key in ipairs({"PS1", "PS2", "PROMPT"}) do
    local val = config.env[key]
    if val and not val:match("%s$") then
      config.env[key] = val .. " "
    end
  end

  -- Docker config block
  if term_meta["docker"] then
    local docker_meta = term_meta["docker"]
    if type(docker_meta) == "table" then
      local docker = {}
      if docker_meta["image"] then docker.image = meta_str(docker_meta["image"]) end
      if docker_meta["pull"] then docker.pull = meta_str(docker_meta["pull"]) end
      if docker_meta["platform"] then docker.platform = meta_str(docker_meta["platform"]) end
      if docker_meta["workdir"] then docker.workdir = meta_str(docker_meta["workdir"]) end
      if docker_meta["user"] then docker.user = meta_str(docker_meta["user"]) end
      if docker_meta["network"] then docker.network = meta_str(docker_meta["network"]) end
      if docker_meta["memory"] then docker.memory = meta_str(docker_meta["memory"]) end
      if docker_meta["cpus"] then docker.cpus = meta_str(docker_meta["cpus"]) end
      if docker_meta["name"] then docker.name = meta_str(docker_meta["name"]) end
      -- ports
      if docker_meta["ports"] then
        docker.ports = {}
        local ports_val = docker_meta["ports"]
        if type(ports_val) == "table" then
          for i = 1, #ports_val do
            table.insert(docker.ports, meta_str(ports_val[i]))
          end
        end
      end
      -- args (escape hatch)
      if docker_meta["args"] then
        docker.args = {}
        local args_val = docker_meta["args"]
        if type(args_val) == "table" then
          for i = 1, #args_val do
            table.insert(docker.args, meta_str(args_val[i]))
          end
        end
      end
      -- volumes: resolve relative host paths
      if docker_meta["volumes"] then
        local cwd = pandoc.system.get_working_directory()
        docker.volumes = {}
        local vols_val = docker_meta["volumes"]
        if type(vols_val) == "table" then
          for i = 1, #vols_val do
            local v = meta_str(vols_val[i])
            local host, rest = v:match("^([^:]+)(:.*)$")
            if host and not host:match("^/") then
              host = cwd .. "/" .. host
            end
            if host then
              table.insert(docker.volumes, host .. rest)
            else
              table.insert(docker.volumes, v)
            end
          end
        end
      end
      -- env under docker block
      if docker_meta["env"] then
        local env_val = docker_meta["env"]
        if type(env_val) == "table" then
          docker.env = {}
          for k, v in pairs(env_val) do
            if type(k) == "string" then
              docker.env[k] = meta_str(v)
            end
          end
        end
      end
      -- Only set config.docker if image is present
      if docker.image then
        config.docker = docker
      end
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



local build_cell = cell_mod.build_cell

function Pandoc(doc)
  local config = extract_config(doc.meta)

  if quarto and quarto.doc and quarto.doc.is_format then
    if quarto.doc.is_format("pdf") or quarto.doc.is_format("latex") then
      config.format = "latex"
    elseif quarto.doc.is_format("typst") then
      config.format = "typst"
    elseif quarto.doc.is_format("gfm") or quarto.doc.is_format("markdown") then
      config.format = "markdown"
    end
  end

  -- Determine format key for style overrides
  local fmt_key = config.format
  if fmt_key == "latex" then fmt_key = "pdf" end
  if quarto and quarto.doc and quarto.doc.is_format and quarto.doc.is_format("revealjs") then
    fmt_key = "revealjs"
  elseif quarto and quarto.doc and quarto.doc.is_format and quarto.doc.is_format("epub") then
    fmt_key = "epub"
  end
  config._fmt_key = fmt_key

  -- Resolve format-specific style overrides
  if config._style_overrides then
    local ov = config._style_overrides[fmt_key]
    if ov then
      if ov.colorscheme then config._colorscheme = ov.colorscheme end
      if ov.colorscheme_light then config._colorscheme_light = ov.colorscheme_light end
      if ov.colorscheme_dark then config._colorscheme_dark = ov.colorscheme_dark end
      if ov.font_family then config.font_family = ov.font_family end
      if ov.font_size then config.font_size = ov.font_size end
      if ov.line_height then config.line_height = ov.line_height end
      if ov.trailing_spaces ~= nil then config.trailing_spaces = ov.trailing_spaces end
      if ov.ansi ~= nil then config.ansi = ov.ansi end
      if ov.spacing ~= nil then config.spacing = ov.spacing end
      if ov.cols then config.cols = ov.cols end
      if ov.rows then config.rows = ov.rows end
    end
    config._style_overrides = nil
  end

  -- Resolve colorscheme colors for Rust
  local _colorscheme = config._colorscheme
  local _colorscheme_light = config._colorscheme_light
  local _colorscheme_dark = config._colorscheme_dark
  config._colorscheme = nil
  config._colorscheme_light = nil
  config._colorscheme_dark = nil

  if _colorscheme then
    local colorscheme_bg, colorscheme_fg = theme.read_colors(_colorscheme)
    if colorscheme_bg then config.colorscheme_bg = colorscheme_bg end
    if colorscheme_fg then config.colorscheme_fg = colorscheme_fg end
  elseif _colorscheme_light then
    local colorscheme_bg, colorscheme_fg = theme.read_colors(_colorscheme_light)
    if colorscheme_bg then config.colorscheme_bg = colorscheme_bg end
    if colorscheme_fg then config.colorscheme_fg = colorscheme_fg end
  end

  -- These fields are only used in Lua; remove before sending to Rust
  local _marker = config.marker
  local _chrome = config._chrome
  local _cache = config._cache
  config.marker = nil
  config._cache = nil
  config._chrome = nil

  local term_positions = {}
  local cells = {}

  for i, block in ipairs(doc.blocks) do
    if is_term_block(block) then
      -- Pass config with marker still set so build_cell can use it
      config.marker = _marker
      local cell = build_cell(block, 0, config)
      config.marker = nil
      local cell_eval = cell._eval
      local cell_include = cell._include
      local cell_colorscheme = cell._colorscheme
      local cell_colorscheme_light = cell._colorscheme_light
      local cell_colorscheme_dark = cell._colorscheme_dark
      local cell_chrome = cell._chrome
      if cell_chrome == nil then cell_chrome = _chrome end
      cell._eval = nil
      cell._include = nil
      cell._colorscheme = nil
      cell._colorscheme_light = nil
      cell._colorscheme_dark = nil
      cell._chrome = nil
      -- For non-HTML formats, resolve cell colorscheme to bg/fg colors
      if cell_colorscheme and config.format ~= "html" then
        local bg, fg = theme.read_colors(cell_colorscheme)
        if bg then cell.options.colorscheme_bg = bg end
        if fg then cell.options.colorscheme_fg = fg end
      end
      if cell_eval then
        local cell_id = #cells + 1
        cell.id = cell_id
        table.insert(cells, cell)
        table.insert(term_positions, {
          block_i = i, cell_i = cell_id, include = cell_include,
          colorscheme = cell_colorscheme,
          colorscheme_light = cell_colorscheme_light,
          colorscheme_dark = cell_colorscheme_dark,
          chrome = cell_chrome,
        })
      else
        -- Strip #| option lines from the code block
        block.text = cell.code
        block.classes = {"bash"}
        doc.blocks[i] = block
      end
    end
  end

  config._fmt_key = nil

  if #cells == 0 then
    return nil
  end

  -- Ensure arrays are encoded as JSON arrays, not objects
  if #config.shell_args == 0 then
    config.shell_args = pandoc.List({})
  end
  if not config.init or #config.init == 0 then
    config.init = pandoc.List({})
  end
  if not config.record or #config.record == 0 then
    config.record = pandoc.List({})
  end
  local env_record = os.getenv("QUARTO_TERM_RECORD")
  if env_record and env_record ~= "" and env_record ~= "1" then
    table.insert(config.record, env_record)
  end
  if config.docker then
    if not config.docker.ports or #config.docker.ports == 0 then
      config.docker.ports = pandoc.List({})
    end
    if not config.docker.volumes or #config.docker.volumes == 0 then
      config.docker.volumes = pandoc.List({})
    end
    if not config.docker.args or #config.docker.args == 0 then
      config.docker.args = pandoc.List({})
    end
  end
  for _, cell in ipairs(cells) do
    if #cell.line_options == 0 then
      cell.line_options = pandoc.List({})
    end
    if #cell.source_lines == 0 then
      cell.source_lines = pandoc.List({})
    end
    if #cell.options.callouts == 0 then
      cell.options.callouts = pandoc.List({})
    end
    if #cell.options.remove == 0 then
      cell.options.remove = pandoc.List({})
    end
    if #cell.options.truncate == 0 then
      cell.options.truncate = pandoc.List({})
    end
  end

  local request = {
    config = config,
    cells = cells,
  }

  local input_json = pandoc.json.encode(request)

  local results
  local use_cache = _cache ~= false

  if use_cache then
    local cache_key = pandoc.utils.sha1(input_json)
    local cache_dir = pandoc.system.get_working_directory() .. "/.quarto-term-cache"
    local cache_file = cache_dir .. "/" .. cache_key .. ".json"
    local cf = io.open(cache_file, "r")
    if cf then
      local cached = cf:read("*a")
      cf:close()
      results = pandoc.json.decode(cached)
      if config.verbose then
        io.stderr:write("quarto-term: cache hit (" .. cache_key:sub(1, 8) .. ")\n")
      end
    else
      local ok, output = pcall(pandoc.pipe, ENGINE, {}, input_json)
      if not ok then
        io.stderr:write("quarto-term: engine error: " .. tostring(output) .. "\n")
        return nil
      end
      results = pandoc.json.decode(output)
      os.execute('mkdir -p "' .. cache_dir .. '"')
      local wf = io.open(cache_file, "w")
      if wf then
        wf:write(output)
        wf:close()
      end
      if config.verbose then
        io.stderr:write("quarto-term: cache miss, stored (" .. cache_key:sub(1, 8) .. ")\n")
      end
    end
  else
    local ok, output = pcall(pandoc.pipe, ENGINE, {}, input_json)
    if not ok then
      io.stderr:write("quarto-term: engine error: " .. tostring(output) .. "\n")
      return nil
    end
    results = pandoc.json.decode(output)
  end

  local raw_format = "html"
  if config.format == "latex" then
    raw_format = "latex"
  elseif config.format == "typst" then
    raw_format = "typst"
  elseif config.format == "markdown" then
    raw_format = "markdown"
  end

  local cell_theme_css = {}

  for _, pos in ipairs(term_positions) do
    local result = results[pos.cell_i]
    if result then
      if result.error and result.error ~= pandoc.json.null and result.error ~= "" then
        io.stderr:write("quarto-term: cell " .. pos.cell_i .. " error: " .. tostring(result.error) .. "\n")
        if os.getenv("QUARTO_TERM_CHECK") then
          error("quarto-term check: assertion failed in cell " .. pos.cell_i)
        end
      end
      if pos.include == false then
        doc.blocks[pos.block_i] = pandoc.RawBlock(raw_format, "")
      else
        local content = result.markup
        if type(content) == "string" and content ~= "" then
          if pos.colorscheme and raw_format == "html" then
            local scope_class = "term-theme-" .. pos.colorscheme
            content = "<div class=\"" .. scope_class .. "\">\n" .. content .. "</div>\n"
            if not cell_theme_css[pos.colorscheme] then
              local css = theme.read_file(pos.colorscheme)
              if css then
                cell_theme_css[pos.colorscheme] = theme.rescope_css(css, "." .. scope_class)
              end
            end
          elseif pos.colorscheme_light and raw_format == "html" then
            local scope_class = "term-theme-cell-" .. pos.cell_i
            content = "<div class=\"" .. scope_class .. "\">\n" .. content .. "</div>\n"
            local key = (pos.colorscheme_light or "") .. "|" .. (pos.colorscheme_dark or "")
            if not cell_theme_css[key] then
              local scoped = ""
              local light_css = theme.read_file(pos.colorscheme_light)
              if light_css then
                scoped = scoped .. theme.rescope_css(light_css, "body.quarto-light ." .. scope_class)
              end
              if pos.colorscheme_dark then
                local dark_css = theme.read_file(pos.colorscheme_dark)
                if dark_css then
                  scoped = scoped .. "\n" .. theme.rescope_css(dark_css, "body.quarto-dark ." .. scope_class)
                end
              end
              if scoped ~= "" then
                cell_theme_css[key] = scoped
              end
            end
          end
          if pos.chrome and raw_format == "html" then
            local chrome_cfg = chrome.normalize(pos.chrome)
            if chrome_cfg then
              content = chrome.wrap(content, chrome_cfg)
            end
          end
          doc.blocks[pos.block_i] = pandoc.RawBlock(raw_format, content)
        else
          doc.blocks[pos.block_i] = pandoc.RawBlock(raw_format, "")
        end
      end
    end
  end

  if os.getenv("QUARTO_TERM_CHECK") then
    local assertions = {}
    for _, pos in ipairs(term_positions) do
      local result = results[pos.cell_i]
      if result and result.recorded_assertions then
        for _, ra in ipairs(result.recorded_assertions) do
          table.insert(assertions, {
            cell_id = pos.cell_i,
            line_index = ra.line_index,
            output = ra.output,
          })
        end
      end
    end
    if #assertions > 0 then
      local input_files = PANDOC_STATE and PANDOC_STATE.input_files
      local src = input_files and input_files[1] and pandoc.utils.stringify(input_files[1])
        or "document"
      local sidecar = src:gsub("%.[^.]+$", "") .. ".assertions.json"
      local sf = io.open(sidecar, "w")
      if sf then
        sf:write(pandoc.json.encode(assertions))
        sf:close()
        io.stderr:write("quarto-term: recorded assertions to " .. sidecar .. "\n")
      end
    end
  end

  if config.format == "latex" then
    if quarto and quarto.doc and quarto.doc.include_text then
      local preamble = "\\usepackage[HTML]{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{fvextra}\n"
      if config.font_family then
        preamble = preamble .. "\\usepackage{fontspec}\n"
      end
      if config.colorscheme_bg then
        preamble = preamble .. "\\definecolor{termbg}{HTML}{" .. config.colorscheme_bg .. "}\n"
      end
      if config.colorscheme_fg then
        preamble = preamble .. "\\definecolor{termfg}{HTML}{" .. config.colorscheme_fg .. "}\n"
      end
      quarto.doc.include_text("in-header", preamble)
    end
  elseif config.format == "typst" then
    -- No special header dependencies needed for typst
  else
    local is_epub = quarto and quarto.doc and quarto.doc.is_format and quarto.doc.is_format("epub")

    if is_epub then
      -- EPUB: inject CSS inline as a RawBlock since add_html_dependency doesn't work
      local css_parts = {}
      local filter_dir = debug.getinfo(1, "S").source:sub(2):match("(.*[/\\])") or "./"
      local f = io.open(filter_dir .. "term.css", "r")
      if f then
        table.insert(css_parts, f:read("*a"))
        f:close()
      end
      if _colorscheme then
        local theme_css = theme.read_file(_colorscheme)
        if theme_css then table.insert(css_parts, theme_css) end
      elseif _colorscheme_light then
        local light_css = theme.read_file(_colorscheme_light)
        if light_css then table.insert(css_parts, light_css) end
      end
      for _, css in pairs(cell_theme_css) do
        table.insert(css_parts, css)
      end
      if #css_parts > 0 then
        -- Insert style block before the first term block
        local style_block = pandoc.RawBlock("html", "<style>\n" .. table.concat(css_parts, "\n") .. "</style>")
        table.insert(doc.blocks, 1, style_block)
      end
    elseif quarto and quarto.doc and quarto.doc.add_html_dependency then
      quarto.doc.add_html_dependency({
        name = "quarto-term",
        version = VERSION,
        stylesheets = { "term.css" },
      })

      if _colorscheme_light and _colorscheme_dark then
        local light_css = theme.read_file(_colorscheme_light)
        local dark_css = theme.read_file(_colorscheme_dark)
        local scoped = ""
        if light_css then
          scoped = scoped .. theme.rescope_css(light_css, "body.quarto-light")
        end
        if dark_css then
          scoped = scoped .. "\n" .. theme.rescope_css(dark_css, "body.quarto-dark")
        end
        if scoped ~= "" then
          quarto.doc.include_text("in-header", "<style>\n" .. scoped .. "</style>")
        end
      elseif _colorscheme then
        quarto.doc.add_html_dependency({
          name = "quarto-term-theme",
          version = VERSION,
          stylesheets = { "themes/" .. _colorscheme .. ".css" },
        })
      end

      -- Inject per-cell theme CSS
      local cell_css_parts = {}
      for _, css in pairs(cell_theme_css) do
        table.insert(cell_css_parts, css)
      end
      if #cell_css_parts > 0 then
        quarto.doc.include_text("in-header", "<style>\n" .. table.concat(cell_css_parts, "\n") .. "</style>")
      end

      -- Copy button script
      quarto.doc.include_text("after-body", [[<script>
(function() {
  function addCopyButtons() {
    var pres = document.querySelectorAll('pre.term-output, pre.term-screen, pre.term-source');
    pres.forEach(function(pre) {
      var wrap = document.createElement('div');
      wrap.className = 'term-container';
      pre.parentNode.insertBefore(wrap, pre);
      wrap.appendChild(pre);
      var btn = document.createElement('button');
      btn.className = 'term-copy-btn';
      btn.setAttribute('aria-label', 'Copy commands to clipboard');
      btn.setAttribute('title', 'Copy commands to clipboard');
      var clipIcon = '<svg width="16" height="16" fill="currentColor" viewBox="0 0 16 16"><path d="M4 1.5H3a2 2 0 0 0-2 2V14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V3.5a2 2 0 0 0-2-2h-1v1h1a1 1 0 0 1 1 1V14a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1V3.5a1 1 0 0 1 1-1h1v-1z"/><path d="M9.5 1a.5.5 0 0 1 .5.5v1a.5.5 0 0 1-.5.5h-3a.5.5 0 0 1-.5-.5v-1a.5.5 0 0 1 .5-.5h3zm-3-1A1.5 1.5 0 0 0 5 1.5v1A1.5 1.5 0 0 0 6.5 4h3A1.5 1.5 0 0 0 11 2.5v-1A1.5 1.5 0 0 0 9.5 0h-3z"/></svg>';
      var checkIcon = '<svg width="16" height="16" fill="currentColor" viewBox="0 0 16 16"><path d="M13.854 3.646a.5.5 0 0 1 0 .708l-7 7a.5.5 0 0 1-.708 0l-3.5-3.5a.5.5 0 1 1 .708-.708L6.5 10.293l6.646-6.647a.5.5 0 0 1 .708 0z"/></svg>';
      btn.innerHTML = clipIcon;
      btn.addEventListener('click', function() {
        var code = pre.querySelector('code');
        var source = code || pre;
        var clone = source.cloneNode(true);
        clone.querySelectorAll('.term-output').forEach(function(el) { el.remove(); });
        var text = clone.innerText.replace(/\n{2,}/g, '\n').trim();
        navigator.clipboard.writeText(text).then(function() {
          btn.innerHTML = checkIcon;
          btn.setAttribute('title', 'Copied!');
          setTimeout(function() {
            btn.innerHTML = clipIcon;
            btn.setAttribute('title', 'Copy commands to clipboard');
          }, 1500);
        });
      });
      wrap.appendChild(btn);
    });
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', addCopyButtons);
  } else {
    addCopyButtons();
  }
})();
</script>]])
    end
  end

  return doc
end
