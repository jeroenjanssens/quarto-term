local FILTER_DIR = debug.getinfo(1, "S").source:sub(2):match("(.*[/\\])") or "./"
local PLAYER_VERSION = "3.9.0"

local player_id_counter = 0
local dependency_injected = false
local injected_themes = {}
local chrome_css_injected = false

local chrome = dofile(FILTER_DIR .. "chrome.lua")

local function inject_dependency()
  if dependency_injected then return end
  dependency_injected = true
  quarto.doc.include_text("in-header",
    '<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/asciinema-player@'
    .. PLAYER_VERSION .. '/dist/bundle/asciinema-player.min.css">\n'
    .. '<script src="https://cdn.jsdelivr.net/npm/asciinema-player@'
    .. PLAYER_VERSION .. '/dist/bundle/asciinema-player.min.js"></script>')
  quarto.doc.add_html_dependency({
    name = "asciicast-player",
    version = "1.0",
    stylesheets = { "asciicast-player.css" },
  })
end

local function transform_theme_vars(css_content)
  local vars = {}
  for line in css_content:gmatch("[^\n]+") do
    local fg = line:match("%-%-term%-fg:%s*(#%x+)")
    if fg then
      table.insert(vars, "  --term-color-foreground: " .. fg .. ";")
    end
    local bg = line:match("%-%-term%-bg:%s*(#%x+)")
    if bg then
      table.insert(vars, "  --term-color-background: " .. bg .. ";")
    end
    local n, color = line:match("%-%-term%-(%d+):%s*(#%x+)")
    if n then
      table.insert(vars, "  --term-color-" .. n .. ": " .. color .. ";")
    end
  end
  if #vars == 0 then return nil end
  return table.concat(vars, "\n")
end

local function transform_theme_css(css_content, theme_name)
  local vars = transform_theme_vars(css_content)
  if not vars then return nil end
  local selector = ".asciinema-player-theme-quarto-term-" .. theme_name
  return selector .. " {\n" .. vars .. "\n}"
end

local function read_theme_file(theme_name)
  local path = FILTER_DIR .. "themes/" .. theme_name .. ".css"
  local f = io.open(path, "r")
  if not f then return nil end
  local content = f:read("*a")
  f:close()
  return content
end

local function inject_theme(theme_name)
  if injected_themes[theme_name] then return end
  injected_themes[theme_name] = true
  local css_content = read_theme_file(theme_name)
  if not css_content then return end
  local transformed = transform_theme_css(css_content, theme_name)
  if transformed then
    quarto.doc.include_text("in-header", "<style>\n" .. transformed .. "</style>")
  end
end

local function inject_dual_theme(light_name, dark_name)
  local key = light_name .. "/" .. dark_name
  if injected_themes[key] then return end
  injected_themes[key] = true

  local theme_class = ".asciinema-player-theme-quarto-term-auto"
  local parts = {}
  local light_css = read_theme_file(light_name)
  if light_css then
    local vars = transform_theme_vars(light_css)
    if vars then
      table.insert(parts, "body.quarto-light " .. theme_class .. " {\n" .. vars .. "\n}")
    end
  end
  local dark_css = read_theme_file(dark_name)
  if dark_css then
    local vars = transform_theme_vars(dark_css)
    if vars then
      table.insert(parts, "body.quarto-dark " .. theme_class .. " {\n" .. vars .. "\n}")
    end
  end

  if #parts > 0 then
    quarto.doc.include_text("in-header", "<style>\n" .. table.concat(parts, "\n") .. "</style>")
  end
end

local function get_doc_colorschemes(meta)
  local term_meta = meta and meta["extensions"] and meta["extensions"]["term"]
  if not term_meta then return nil, nil end
  local style = term_meta["style"]
  if not style then return nil, nil end
  local html_style = style["html"]
  if not html_style then return nil, nil end
  local light = html_style["colorscheme-light"] or html_style["colorscheme_light"]
  local dark = html_style["colorscheme-dark"] or html_style["colorscheme_dark"]
  if light then light = pandoc.utils.stringify(light) end
  if dark then dark = pandoc.utils.stringify(dark) end
  return light, dark
end

local OPTION_MAP = {
  ["cols"] = "cols",
  ["rows"] = "rows",
  ["autoplay"] = "autoplay",
  ["preload"] = "preload",
  ["loop"] = "loop",
  ["start-at"] = "startAt",
  ["speed"] = "speed",
  ["idle-time-limit"] = "idleTimeLimit",
  ["poster"] = "poster",
  ["audio-url"] = "audioUrl",
  ["fit"] = "fit",
  ["controls"] = "controls",
  ["keystroke-overlay"] = "keystrokeOverlay",
  ["markers"] = "markers",
  ["pause-on-markers"] = "pauseOnMarkers",
  ["font-size"] = "terminalFontSize",
  ["font-family"] = "terminalFontFamily",
  ["line-height"] = "terminalLineHeight",
  ["cursor-mode"] = "cursorMode",
}

local BOOLEAN_OPTS = {
  autoplay = true, preload = true, ["keystroke-overlay"] = true, ["pause-on-markers"] = true,
}

local NUMBER_OPTS = {
  cols = true, rows = true, speed = true, ["idle-time-limit"] = true, ["line-height"] = true,
}

local function format_js_value(key, value)
  if BOOLEAN_OPTS[key] then
    return value == "true" and "true" or "false"
  end
  if NUMBER_OPTS[key] then
    return value
  end
  if key == "loop" then
    if value == "true" then return "true" end
    if value == "false" then return "false" end
    return value
  end
  if key == "controls" then
    if value == "true" then return "true" end
    if value == "false" then return "false" end
    return '"' .. value .. '"'
  end
  if key == "markers" then
    return value
  end
  return '"' .. value:gsub('"', '\\"') .. '"'
end

return {
  ["asciicast-player"] = function(args, kwargs, meta)
    if not quarto.doc.is_format("html:js") then
      return pandoc.Null()
    end

    inject_dependency()

    local src = kwargs["src"]
    if not src and #args > 0 then
      src = pandoc.utils.stringify(args[1])
    end
    if not src then
      io.stderr:write("asciicast-player: missing required 'src' argument\n")
      return pandoc.Null()
    end
    if type(src) ~= "string" then
      src = pandoc.utils.stringify(src)
    end

    player_id_counter = player_id_counter + 1
    local player_id = "asciicast-player-" .. player_id_counter

    local theme_name = kwargs["theme"]
    if theme_name then
      theme_name = pandoc.utils.stringify(theme_name)
      if theme_name == "" then theme_name = nil end
    end
    if theme_name then
      inject_theme(theme_name)
    end

    local js_theme
    if theme_name then
      js_theme = "quarto-term-" .. theme_name
    else
      local light, dark = get_doc_colorschemes(meta)
      if light and dark then
        inject_dual_theme(light, dark)
        js_theme = "quarto-term-auto"
      elseif light then
        inject_theme(light)
        js_theme = "quarto-term-" .. light
      elseif dark then
        inject_theme(dark)
        js_theme = "quarto-term-" .. dark
      end
    end

    local opts = {}
    for kebab, camel in pairs(OPTION_MAP) do
      local val = kwargs[kebab]
      if val then
        val = pandoc.utils.stringify(val)
        if val ~= "" then
          table.insert(opts, "  " .. camel .. ": " .. format_js_value(kebab, val))
        end
      end
    end
    if js_theme then
      table.insert(opts, '  theme: "' .. js_theme .. '"')
    end

    local opts_str = ""
    if #opts > 0 then
      opts_str = ", {\n" .. table.concat(opts, ",\n") .. "\n}"
    end

    local chrome_cfg = nil
    local chrome_val = kwargs["chrome"]
    if chrome_val then
      chrome_val = pandoc.utils.stringify(chrome_val)
      if chrome_val == "true" or chrome_val == "" then
        chrome_cfg = { os = "macos", shadow = true }
      elseif chrome_val ~= "false" then
        chrome_cfg = { os = chrome_val, shadow = true }
      end
    end
    local chrome_os = kwargs["chrome-os"]
    if chrome_os then
      chrome_os = pandoc.utils.stringify(chrome_os)
      if chrome_os ~= "" then
        if not chrome_cfg then chrome_cfg = { shadow = true } end
        chrome_cfg.os = chrome_os
      end
    end
    local chrome_title = kwargs["chrome-title"]
    if chrome_title then
      chrome_title = pandoc.utils.stringify(chrome_title)
      if chrome_title ~= "" then
        if not chrome_cfg then chrome_cfg = { os = "macos", shadow = true } end
        chrome_cfg.title = chrome_title
      end
    end
    local chrome_color = kwargs["chrome-color"]
    if chrome_color then
      chrome_color = pandoc.utils.stringify(chrome_color)
      if chrome_color ~= "" then
        if not chrome_cfg then chrome_cfg = { os = "macos", shadow = true } end
        chrome_cfg.color = chrome_color
      end
    end
    local chrome_shadow = kwargs["chrome-shadow"]
    if chrome_shadow then
      chrome_shadow = pandoc.utils.stringify(chrome_shadow)
      if chrome_shadow == "false" then
        if not chrome_cfg then chrome_cfg = { os = "macos" } end
        chrome_cfg.shadow = false
      end
    end
    if chrome_cfg then
      chrome_cfg = chrome.normalize(chrome_cfg)
      if not chrome_css_injected then
        chrome_css_injected = true
        local css_f = io.open(FILTER_DIR .. "term.css", "r")
        if css_f then
          css_f:close()
          quarto.doc.add_html_dependency({
            name = "quarto-term-chrome",
            version = "1.0",
            stylesheets = { "term.css" },
          })
        end
      end
    end

    local player_html = '<div id="' .. player_id .. '"></div>'
    local inner = chrome_cfg
      and chrome.wrap(player_html, chrome_cfg)
      or player_html

    local html = '<div class="asciicast-player-container">\n'
      .. '  ' .. inner .. '\n'
      .. '</div>\n'
      .. '<script>\n'
      .. 'AsciinemaPlayer.create("' .. src:gsub('"', '\\"') .. '", '
      .. 'document.getElementById("' .. player_id .. '")'
      .. opts_str .. ');\n'
      .. '</script>'

    return pandoc.RawBlock("html", html)
  end
}
