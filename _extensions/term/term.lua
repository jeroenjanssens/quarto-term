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

local config_mod = dofile(FILTER_DIR .. "config.lua")

local extract_config = config_mod.extract_config

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
    local cache_key = pandoc.utils.sha1(VERSION .. "\n" .. input_json)
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
