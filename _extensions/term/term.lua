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
    project_root .. "../target/release/quarto-term",
    project_root .. "../target/debug/quarto-term",
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
  local nested_key = nil

  local nest_stack = {}

  for line in text:gmatch("([^\n]*)\n?") do
    if in_options and line:match("^#|%s*") then
      local content = line:match("^#| (.*)$") or line:match("^#|(.*)$")
      local indent = #(content:match("^(%s*)") or "")
      local trimmed = content:match("^%s*(.-)%s*$")

      if #nest_stack > 0 and indent == 0 then
        nest_stack = {}
      end
      while #nest_stack > 0 and indent <= nest_stack[#nest_stack].indent do
        table.remove(nest_stack)
      end

      if trimmed == "" then goto continue end

      local k, v = trimmed:match("^(%S+):%s*(.+)$")
      if k then
        local parsed_value
        local list_match = v:match("^%[(.*)%]$")
        if list_match then
          parsed_value = {}
          for item in list_match:gmatch("[^,]+") do
            table.insert(parsed_value, coerce_value(item))
          end
        else
          parsed_value = coerce_value(v)
        end
        local tbl = cell_opts
        for _, frame in ipairs(nest_stack) do
          tbl = tbl[frame.key]
        end
        if k:find("%.") then
          local segments = {}
          for segment in k:gmatch("([^%.]+)") do
            table.insert(segments, segment)
          end
          for i = 1, #segments - 1 do
            tbl[segments[i]] = tbl[segments[i]] or {}
            tbl = tbl[segments[i]]
          end
          tbl[segments[#segments]] = parsed_value
        else
          tbl[k] = parsed_value
        end
      else
        local block_key = trimmed:match("^(%S+):%s*$")
        if block_key then
          local tbl = cell_opts
          for _, frame in ipairs(nest_stack) do
            tbl = tbl[frame.key]
          end
          tbl[block_key] = tbl[block_key] or {}
          table.insert(nest_stack, { key = block_key, indent = indent })
        end
      end
    else
      in_options = false
      table.insert(code_lines, line)
    end
    ::continue::
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
  if term_meta["timeout"] then config.timeout = meta_num(term_meta["timeout"]) or config.timeout end
  if term_meta["init"] then
    local init_val = term_meta["init"]
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
        -- Inline command(s) — split multi-line blocks
        for line in entry:gmatch("[^\n]+") do
          local trimmed = line:match("^%s*(.-)%s*$")
          if trimmed and trimmed ~= "" then
            table.insert(config.init, trimmed)
          end
        end
      end
    end
  end
  if term_meta["verbose"] ~= nil then config.verbose = meta_bool(term_meta["verbose"]) end
  if term_meta["spacing"] ~= nil then config.spacing = meta_bool(term_meta["spacing"]) end

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
    if style_meta["cols"] then s.cols = meta_num(style_meta["cols"]) end
    if style_meta["rows"] then s.rows = meta_num(style_meta["rows"]) end
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
    if style.cols then config.cols = style.cols end
    if style.rows then config.rows = style.rows end
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

  local eval = cell_opts["eval"]
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

  local label = cell_opts["label"]

  -- Cell-level style (supports format-specific overrides)
  local cell_style = cell_opts["style"] or {}
  local cell_colorscheme = nil
  local cell_colorscheme_light = nil
  local cell_colorscheme_dark = nil
  if type(cell_style) == "table" then
    -- Apply base style
    if cell_style["colorscheme"] then cell_colorscheme = cell_style["colorscheme"] end
    local csl = cell_style["colorscheme-light"] or cell_style["colorscheme_light"]
    if csl then cell_colorscheme_light = csl end
    local csd = cell_style["colorscheme-dark"] or cell_style["colorscheme_dark"]
    if csd then cell_colorscheme_dark = csd end
    local ff = cell_style["font-family"] or cell_style["font_family"]
    if ff then options.font_family = ff end
    local fs = cell_style["font-size"] or cell_style["font_size"]
    if fs then options.font_size = fs end
    local lh = cell_style["line-height"] or cell_style["line_height"]
    if lh then options.line_height = lh end
    if cell_style["ansi"] ~= nil then options.ansi = cell_style["ansi"] end
    local ts = cell_style["trailing-spaces"] or cell_style["trailing_spaces"]
    if ts ~= nil then options.trailing_spaces = ts end
    -- Apply format-specific overrides
    local fmt_key = config._fmt_key
    if fmt_key and type(cell_style[fmt_key]) == "table" then
      local ov = cell_style[fmt_key]
      if ov["colorscheme"] then cell_colorscheme = ov["colorscheme"] end
      local ov_csl = ov["colorscheme-light"] or ov["colorscheme_light"]
      if ov_csl then cell_colorscheme_light = ov_csl end
      local ov_csd = ov["colorscheme-dark"] or ov["colorscheme_dark"]
      if ov_csd then cell_colorscheme_dark = ov_csd end
      local ov_ff = ov["font-family"] or ov["font_family"]
      if ov_ff then options.font_family = ov_ff end
      local ov_fs = ov["font-size"] or ov["font_size"]
      if ov_fs then options.font_size = ov_fs end
      local ov_lh = ov["line-height"] or ov["line_height"]
      if ov_lh then options.line_height = ov_lh end
      if ov["ansi"] ~= nil then options.ansi = ov["ansi"] end
      local ov_ts = ov["trailing-spaces"] or ov["trailing_spaces"]
      if ov_ts ~= nil then options.trailing_spaces = ov_ts end
    end
  end

  return {
    id = cell_id,
    code = code,
    label = label,
    options = options,
    line_options = line_options,
    source_lines = source_lines,
    _eval = eval ~= false,
    _include = include ~= false,
    _colorscheme = cell_colorscheme,
    _colorscheme_light = cell_colorscheme_light,
    _colorscheme_dark = cell_colorscheme_dark,
  }
end

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
    local theme_bg, theme_fg = read_theme_colors(_colorscheme)
    if theme_bg then config.theme_bg = theme_bg end
    if theme_fg then config.theme_fg = theme_fg end
  elseif _colorscheme_light then
    local theme_bg, theme_fg = read_theme_colors(_colorscheme_light)
    if theme_bg then config.theme_bg = theme_bg end
    if theme_fg then config.theme_fg = theme_fg end
  end

  -- These fields are only used in Lua; remove before sending to Rust
  local _marker = config.marker
  config.marker = nil

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
      cell._eval = nil
      cell._include = nil
      cell._colorscheme = nil
      cell._colorscheme_light = nil
      cell._colorscheme_dark = nil
      -- For non-HTML formats, resolve cell colorscheme to bg/fg colors
      if cell_colorscheme and config.format ~= "html" then
        local bg, fg = read_theme_colors(cell_colorscheme)
        if bg then cell.options.theme_bg = bg end
        if fg then cell.options.theme_fg = fg end
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
      end
      if pos.include == false then
        doc.blocks[pos.block_i] = pandoc.Null()
      else
        local content = result.html
        if type(content) == "string" and content ~= "" then
          if pos.colorscheme and raw_format == "html" then
            local scope_class = "term-theme-" .. pos.colorscheme
            content = "<div class=\"" .. scope_class .. "\">\n" .. content .. "</div>\n"
            if not cell_theme_css[pos.colorscheme] then
              local css = read_theme_file(pos.colorscheme)
              if css then
                cell_theme_css[pos.colorscheme] = rescope_theme_css(css, "." .. scope_class)
              end
            end
          end
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
      if config.font_family then
        preamble = preamble .. "\\usepackage{fontspec}\n"
      end
      if config.theme_bg then
        preamble = preamble .. "\\definecolor{termbg}{HTML}{" .. config.theme_bg .. "}\n"
      end
      if config.theme_fg then
        preamble = preamble .. "\\definecolor{termfg}{HTML}{" .. config.theme_fg .. "}\n"
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
        local theme_css = read_theme_file(_colorscheme)
        if theme_css then table.insert(css_parts, theme_css) end
      elseif _colorscheme_light then
        local light_css = read_theme_file(_colorscheme_light)
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
        version = "0.2.0",
        stylesheets = { "term.css" },
      })

      if _colorscheme_light and _colorscheme_dark then
        local light_css = read_theme_file(_colorscheme_light)
        local dark_css = read_theme_file(_colorscheme_dark)
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
      elseif _colorscheme then
        quarto.doc.add_html_dependency({
          name = "quarto-term-theme",
          version = "0.2.0",
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
    end
  end

  return doc
end
