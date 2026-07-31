local function detect_platform()
  local os_name = pandoc.system.os
  local arch = pandoc.system.arch

  if os_name == "darwin" then
    if arch == "aarch64" or arch == "arm64" then
      return "aarch64-apple-darwin"
    else
      return "x86_64-apple-darwin"
    end
  elseif os_name == "linux" then
    if arch == "aarch64" or arch == "arm64" then
      return "aarch64-unknown-linux-gnu"
    else
      return "x86_64-unknown-linux-gnu"
    end
  elseif os_name == "windows" or os_name == "mingw32" then
    return "x86_64-pc-windows-msvc"
  end
  return nil
end

local function find_engine()
  local filter_dir = debug.getinfo(1, "S").source:sub(2):match("(.*[/\\])") or "./"
  local project_root = filter_dir:match("(.*/?)_extensions/term/") or "./"

  -- 1. Check for platform-specific binary bundled with the extension
  local platform = detect_platform()
  if platform then
    local ext = platform:match("windows") and ".exe" or ""
    local bundled = filter_dir .. "bin/quarto-term-" .. platform .. ext
    local f = io.open(bundled, "r")
    if f then
      f:close()
      return bundled
    end
  end

  -- 2. Check for development build (cargo build)
  local dev_candidates = {
    project_root .. "target/release/quarto-term",
    project_root .. "target/debug/quarto-term",
    "./target/release/quarto-term",
    "./target/debug/quarto-term",
  }
  for _, path in ipairs(dev_candidates) do
    local f = io.open(path, "r")
    if f then
      f:close()
      return path
    end
  end

  -- 3. Fall back to PATH
  return "quarto-term"
end

local ENGINE = find_engine()

local function read_theme_file(theme_name)
  if not theme_name then return nil end
  local filter_dir = debug.getinfo(1, "S").source:sub(2):match("(.*[/\\])") or "./"
  local path = filter_dir .. "themes/" .. theme_name .. ".css"
  local f = io.open(path, "r")
  if not f then return nil end
  local content = f:read("*a")
  f:close()
  return content
end

local function read_theme_colors(theme_name)
  local content = read_theme_file(theme_name)
  if not content then return nil, nil end
  local bg = content:match("%-%-term%-bg:%s*#(%x+)")
  local fg = content:match("%-%-term%-fg:%s*#(%x+)")
  return bg, fg
end

local function rescope_theme_css(css_content, selector)
  -- Replace `:root {` with `selector {`
  return css_content:gsub(":root%s*{", selector .. " {")
end

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
      -- Pattern matches keys with letters, underscores, or hyphens for compat
      for k, v in opts_str:gmatch("(%a[%a_%-]*):%s*([^,]+),?%s*") do
        -- Normalize hyphens to underscores so Rust structs (snake_case) receive correct keys
        local norm_k = k:gsub("%-", "_")
        opts[norm_k] = coerce_value(v)
      end
      table.insert(parsed_lines, code_part)
      table.insert(line_options, opts)
    else
      table.insert(parsed_lines, raw_line)
    end
  end

  return cell_opts, table.concat(parsed_lines, "\n"), line_options, code_lines
end

local function resolve_init_path(path)
  if path:sub(1, 1) == "/" then
    return path
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
  return path
end

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

  if term_meta["prompt"] then
    config.prompt = meta_str(term_meta["prompt"])
  end

  -- prompt-regex: raw regex override for prompt matching
  local prompt_regex_val = term_meta["prompt-regex"] or term_meta["prompt_regex"]
  if prompt_regex_val then
    config.prompt_regex = meta_str(prompt_regex_val)
  end
  if term_meta["cols"] then config.cols = meta_num(term_meta["cols"]) or config.cols end
  if term_meta["rows"] then config.rows = meta_num(term_meta["rows"]) or config.rows end
  if term_meta["ansi"] ~= nil then config.ansi = meta_bool(term_meta["ansi"]) end
  if term_meta["timeout"] then config.timeout = meta_num(term_meta["timeout"]) or config.timeout end
  if term_meta["init"] then
    local init_val = term_meta["init"]
    local init_list = {}
    if type(init_val) == "table" and init_val.t == nil then
      for i = 1, #init_val do
        table.insert(init_list, meta_str(init_val[i]))
      end
    else
      table.insert(init_list, meta_str(init_val))
    end
    config.init = {}
    for _, entry in ipairs(init_list) do
      table.insert(config.init, resolve_init_path(entry))
    end
  end
  if term_meta["verbose"] ~= nil then config.verbose = meta_bool(term_meta["verbose"]) end
  if term_meta["spacing"] ~= nil then config.spacing = meta_bool(term_meta["spacing"]) end
  if term_meta["theme"] then
    local theme_val = term_meta["theme"]
    if type(theme_val) == "table" and theme_val.t == nil then
      -- Map with light/dark keys
      if theme_val["light"] then config.theme_light = meta_str(theme_val["light"]) end
      if theme_val["dark"] then config.theme_dark = meta_str(theme_val["dark"]) end
    else
      config.theme = meta_str(theme_val)
    end
  end

  -- Accept both kebab-case (preferred) and snake_case (compat) for theme-bg / theme-fg
  local theme_bg_val = term_meta["theme-bg"] or term_meta["theme_bg"]
  if theme_bg_val then config.theme_bg = meta_str(theme_bg_val) end
  local theme_fg_val = term_meta["theme-fg"] or term_meta["theme_fg"]
  if theme_fg_val then config.theme_fg = meta_str(theme_fg_val) end

  -- Accept both kebab-case (preferred) and snake_case (compat) for trailing-spaces
  local trailing_spaces_val = term_meta["trailing-spaces"] or term_meta["trailing_spaces"]
  if trailing_spaces_val ~= nil then
    config.trailing_spaces = meta_bool(trailing_spaces_val)
  end

  -- marker option (used only in Lua, not sent to Rust)
  if term_meta["marker"] then
    config.marker = meta_str(term_meta["marker"])
  end

  if term_meta["fontsize"] then
    local fs = term_meta["fontsize"]
    if type(fs) == "table" and fs.t == nil then
      -- Per-format map: { html: "0.8em", pdf: "0.7em" }
      config._fontsize_map = {}
      for k, v in pairs(fs) do
        if type(k) == "string" then
          config._fontsize_map[k] = meta_str(v)
        end
      end
    else
      config.fontsize = meta_str(fs)
    end
  end
  if term_meta["typing"] then
    local typing_val = term_meta["typing"]
    if meta_bool(typing_val) == false then
      config.typing = false
    elseif type(typing_val) == "table" and typing_val.t == nil then
      config.typing = {}
      if typing_val["mode"] then config.typing.mode = meta_str(typing_val["mode"]) end
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

  -- Pandoc strips trailing spaces from YAML values; restore for prompt vars
  for _, key in ipairs({"PS1", "PS2", "PROMPT"}) do
    local val = config.env[key]
    if val and not val:match("%s$") then
      config.env[key] = val .. " "
    end
  end

  -- Set a known PS2 so we can detect continuation prompts
  if not config.env["PS2"] then
    config.env["PS2"] = "> "
  end
  config.ps2 = config.env["PS2"]

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
  -- Determine the line marker: chunk-level overrides document-level, default "#!"
  local line_marker = "#!"
  if config.marker then
    line_marker = config.marker
  end

  local cell_opts, code, line_options, source_lines = parse_cell_options(block.text, line_marker)

  -- Allow chunk-level marker override (read before other opts since it affects parsing,
  -- but parsing already happened above with the doc-level marker; this override applies
  -- to future use if re-parsing is needed -- for now just store it)
  if cell_opts["marker"] then
    -- Re-parse with the chunk-level marker if it differs
    local chunk_marker = cell_opts["marker"]
    if chunk_marker ~= line_marker then
      cell_opts, code, line_options, source_lines = parse_cell_options(block.text, chunk_marker)
    end
  end

  local options = {
    echo = "terminal",
    output = true,
    fullscreen = false,
    keep_last_prompt = false,
    callouts = pandoc.List({}),
    remove = pandoc.List({}),
    highlight = "bash",
  }

  local include = cell_opts["include"]
  if include == false then
    options.echo = "false"
    options.output = false
  end
  if cell_opts["echo"] ~= nil then options.echo = cell_opts["echo"] end
  if cell_opts["output"] ~= nil then options.output = cell_opts["output"] end
  if cell_opts["fullscreen"] ~= nil then options.fullscreen = cell_opts["fullscreen"] end
  if cell_opts["scroll"] ~= nil then options.scroll = cell_opts["scroll"] end

  -- Accept both kebab-case (preferred) and snake_case (compat) for keep-last-prompt
  local klp = cell_opts["keep-last-prompt"]
  if klp == nil then klp = cell_opts["keep_last_prompt"] end
  if klp ~= nil then options.keep_last_prompt = klp end

  if cell_opts["ansi"] ~= nil then options.ansi = cell_opts["ansi"] end
  if cell_opts["spacing"] ~= nil then options.spacing = cell_opts["spacing"] end
  if cell_opts["typing"] ~= nil then
    if cell_opts["typing"] == false then
      options.typing = false
    elseif cell_opts["typing"] == true then
      options.typing = {}
    end
  end
  if cell_opts["timeout"] ~= nil then options.timeout = cell_opts["timeout"] end
  if cell_opts["hold"] ~= nil then options.hold = cell_opts["hold"] end
  if cell_opts["callouts"] ~= nil then options.callouts = cell_opts["callouts"] end
  if cell_opts["remove"] ~= nil then options.remove = cell_opts["remove"] end
  if cell_opts["highlight"] ~= nil then options.highlight = cell_opts["highlight"] end

  if cell_opts["literal"] ~= nil then options.literal = cell_opts["literal"] end
  if cell_opts["delay"] ~= nil then options.delay = cell_opts["delay"] end

  -- Accept both kebab-case (preferred) and snake_case (compat) for trailing-spaces
  local ts = cell_opts["trailing-spaces"]
  if ts == nil then ts = cell_opts["trailing_spaces"] end
  if ts ~= nil then options.trailing_spaces = ts end

  local label = cell_opts["label"]

  return {
    id = cell_id,
    code = code,
    label = label,
    options = options,
    line_options = line_options,
    source_lines = source_lines,
    _include = include ~= false,
  }
end

function Pandoc(doc)
  local config = extract_config(doc.meta)

  if quarto and quarto.doc and quarto.doc.is_format then
    if quarto.doc.is_format("pdf") or quarto.doc.is_format("latex") then
      config.format = "latex"
    elseif quarto.doc.is_format("gfm") or quarto.doc.is_format("markdown") then
      config.format = "markdown"
    end
  end

  -- Resolve theme colors for Rust (used in non-ANSI rendering)
  if config.theme then
    local theme_bg, theme_fg = read_theme_colors(config.theme)
    if theme_bg then config.theme_bg = theme_bg end
    if theme_fg then config.theme_fg = theme_fg end
  elseif config.theme_light then
    local theme_bg, theme_fg = read_theme_colors(config.theme_light)
    if theme_bg then config.theme_bg = theme_bg end
    if theme_fg then config.theme_fg = theme_fg end
  end

  -- Resolve per-format fontsize
  if config._fontsize_map then
    local fmt_key = config.format == "latex" and "pdf" or config.format
    config.fontsize = config._fontsize_map[fmt_key]
    config._fontsize_map = nil
  end

  -- These fields are only used in Lua; remove before sending to Rust
  local _marker = config.marker
  local _theme = config.theme
  local _theme_light = config.theme_light
  local _theme_dark = config.theme_dark
  config.marker = nil
  config.theme = nil
  config.theme_light = nil
  config.theme_dark = nil

  local term_positions = {}
  local cells = {}

  for i, block in ipairs(doc.blocks) do
    if is_term_block(block) then
      local cell_id = #cells + 1
      -- Pass config with marker still set so build_cell can use it
      config.marker = _marker
      local cell = build_cell(block, cell_id, config)
      config.marker = nil
      local cell_include = cell._include
      cell._include = nil
      table.insert(cells, cell)
      table.insert(term_positions, { block_i = i, cell_i = cell_id, include = cell_include })
    end
  end

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

  local raw_format = "html"
  if config.format == "latex" then
    raw_format = "latex"
  elseif config.format == "markdown" then
    raw_format = "markdown"
  end

  for _, pos in ipairs(term_positions) do
    local result = results[pos.cell_i]
    if result then
      if result.error and result.error ~= pandoc.json.null and result.error ~= "" then
        io.stderr:write("quarto-term: cell " .. pos.cell_i .. " error: " .. tostring(result.error) .. "\n")
      end
      if pos.include == false then
        doc.blocks[pos.block_i] = pandoc.Null()
      else
        local content = result.html
        if type(content) == "string" and content ~= "" then
          doc.blocks[pos.block_i] = pandoc.RawBlock(raw_format, content)
        else
          doc.blocks[pos.block_i] = pandoc.Null()
        end
      end
    end
  end

  if config.format == "latex" then
    if quarto and quarto.doc and quarto.doc.include_text then
      local preamble = "\\usepackage[HTML]{xcolor}\n\\usepackage{tcolorbox}\n\\usepackage{fvextra}\n"
      if config.theme_bg then
        preamble = preamble .. "\\definecolor{termbg}{HTML}{" .. config.theme_bg .. "}\n"
      end
      if config.theme_fg then
        preamble = preamble .. "\\definecolor{termfg}{HTML}{" .. config.theme_fg .. "}\n"
      end
      quarto.doc.include_text("in-header", preamble)
    end
  else
    if quarto and quarto.doc and quarto.doc.add_html_dependency then
      quarto.doc.add_html_dependency({
        name = "quarto-term",
        version = "0.2.0",
        stylesheets = { "term.css" },
      })

      if _theme_light and _theme_dark then
        -- Dual theme: scope each theme's CSS vars under body.quarto-light / body.quarto-dark
        local light_css = read_theme_file(_theme_light)
        local dark_css = read_theme_file(_theme_dark)
        local scoped = ""
        if light_css then
          scoped = scoped .. rescope_theme_css(light_css, "body.quarto-light")
        end
        if dark_css then
          scoped = scoped .. "\n" .. rescope_theme_css(dark_css, "body.quarto-dark")
        end
        if scoped ~= "" then
          quarto.doc.include_text("in-header", "<style>\n" .. scoped .. "</style>")
        end
      elseif _theme then
        -- Single theme: include the theme file directly
        quarto.doc.add_html_dependency({
          name = "quarto-term-theme",
          version = "0.2.0",
          stylesheets = { "themes/" .. _theme .. ".css" },
        })
      end
    end
  end

  return doc
end
