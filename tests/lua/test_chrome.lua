-- Test chrome.lua (window chrome around terminal blocks)
local h = dofile(debug.getinfo(1, "S").source:sub(2):match("(.*[/\\])") .. "helpers.lua")
local chrome = dofile(h.project_root() .. "_extensions/term/chrome.lua")

-- normalize
do
  h.assert_nil(chrome.normalize(nil), "nil: no chrome")
  h.assert_nil(chrome.normalize(false), "false: no chrome")
  local c = chrome.normalize(true)
  h.assert_eq(c.os, "macos", "true: macOS")
  h.assert_nil(c.title, "true: no title")
  h.assert_eq(c.shadow, true, "true: shadow")

  c = chrome.normalize("nano")
  h.assert_eq(c.title, "nano", "string: the title")
  h.assert_eq(c.os, "macos", "string: macOS")
  h.assert_eq(c.shadow, true, "string: shadow")

  c = chrome.normalize({ title = "less", os = "linux", shadow = false, color = "#333" })
  h.assert_eq(c.title, "less", "map: title")
  h.assert_eq(c.os, "linux", "map: os")
  h.assert_eq(c.shadow, false, "map: shadow off")
  h.assert_eq(c.color, "#333", "map: color")
  h.assert_eq(chrome.normalize({}).os, "macos", "empty map: macOS")
end

-- wrap
do
  local html = chrome.wrap("<pre>x</pre>", chrome.normalize("nano"))
  h.assert_true(html:find('<div class="term-chrome term-chrome-macos">', 1, true) ~= nil, "macOS wrapper")
  h.assert_true(html:find('<span class="term-chrome-title">nano</span>', 1, true) ~= nil, "title")
  h.assert_true(html:find("<pre>x</pre>", 1, true) ~= nil, "content kept")
  h.assert_eq(select(2, html:gsub("term%-chrome%-dot", "")), 3, "three dots")

  html = chrome.wrap("<pre>x</pre>", chrome.normalize(true))
  h.assert_true(html:find("term-chrome-title", 1, true) == nil, "no title span without a title")

  html = chrome.wrap("<pre>x</pre>", chrome.normalize({ title = 'a <b> & "c" {{< x >}}' }))
  h.assert_true(html:find('a &lt;b&gt; &amp; &quot;c&quot; &#123;&#123;&lt; x &gt;}}', 1, true) ~= nil, "title escaped")
  h.assert_true(html:find("{{<", 1, true) == nil, "no shortcode opener")

  html = chrome.wrap("<pre>x</pre>", chrome.normalize({ os = "windows", title = "PowerShell" }))
  h.assert_true(html:find("term-chrome-windows", 1, true) ~= nil, "Windows wrapper")
  h.assert_true(html:find("term-chrome-btn", 1, true) ~= nil, "Windows buttons")

  html = chrome.wrap("<pre>x</pre>", chrome.normalize({ shadow = false }))
  h.assert_true(html:find('style="box-shadow:none"', 1, true) ~= nil, "shadow off")
end

h.finish("test_chrome")
