-- Test callout_list.lua (uses the real pandoc module, so no stubs)
local h = dofile(debug.getinfo(1, "S").source:sub(2):match("(.*[/\\])") .. "helpers.lua")
local cl = dofile(h.project_root() .. "_extensions/term/callout_list.lua")

local function join(t) return table.concat(t, "|") end

-- symbols: one per character, put into the format
do
  h.assert_eq(join(cl.symbols("➊➋➌", "{}")), "➊|➋|➌", "multibyte characters, plain format")
  h.assert_eq(join(cl.symbols("abc", "({})")), "(a)|(b)|(c)", "format around the value")
  h.assert_eq(join(cl.symbols(nil, nil)), "<1>|<2>|<3>|<4>|<5>|<6>|<7>|<8>|<9>", "defaults like the binary")
  h.assert_eq(join(cl.symbols({ "start", "end" }, "[{}]")), "[start]|[end]", "list of strings")
  h.assert_eq(join(cl.symbols("1", "{}%")), "1%", "a % in the format is literal")
  h.assert_eq(join(cl.symbols({ "%d" }, "{}")), "%d", "a % in a value is literal")
  h.assert_eq(join(cl.symbols("x", "{}-{}")), "x-x", "every {} is replaced")
end

-- css_string: a valid CSS string literal
do
  h.assert_eq(cl.css_string("➊"), '"➊"', "plain")
  h.assert_eq(cl.css_string('a"b'), '"a\\"b"', "quote escaped")
  h.assert_eq(cl.css_string("a\\b"), '"a\\\\b"', "backslash escaped")
end

-- class_name: stable, and different for different symbols
do
  local a = cl.class_name({ "➊", "➋" })
  h.assert_eq(a, cl.class_name({ "➊", "➋" }), "same symbols, same class")
  h.assert_true(a ~= cl.class_name({ "➊", "➌" }), "different symbols, different class")
  h.assert_true(a ~= cl.class_name({ "➊➋" }), "symbols are separated")
  h.assert_true(a:match("^term%-callouts%-%x%x%x%x%x%x%x%x$") ~= nil, "class format")
end

-- css: a @counter-style and the rule that applies it
do
  local symbols = { "➊", "➋" }
  local name = cl.class_name(symbols)
  local css = cl.css(symbols)
  h.assert_true(css:find("@counter-style " .. name .. " {", 1, true) ~= nil, "counter style")
  h.assert_true(css:find('symbols: "➊" "➋";', 1, true) ~= nil, "symbols")
  h.assert_true(css:find("system: fixed;", 1, true) ~= nil, "fixed: numbers beyond the symbols fall back to decimal")
  h.assert_true(css:find(".term-callout-list." .. name .. " > ol {", 1, true) ~= nil, "selector")
  h.assert_true(css:find("list-style-type: " .. name .. ";", 1, true) ~= nil, "list-style-type")
end

local symbols = { "➊", "➋" }
local class = cl.class_name(symbols)
local function list(...) return pandoc.OrderedList({ ... }) end
local function item(text) return { pandoc.Plain({ pandoc.Str(text) }) } end
local output = pandoc.RawBlock("html", "<pre>output</pre>")

-- wrap: the list right after a marker gets wrapped; the marker goes
do
  local blocks = pandoc.Blocks({ output, cl.marker(symbols), list(item("a"), item("b")), pandoc.Para("after") })
  local out = cl.wrap(blocks)
  h.assert_eq(#out, 3, "output, wrapped list, paragraph")
  h.assert_eq(out[1].t, "RawBlock", "output kept")
  h.assert_eq(out[2].t, "Div", "list wrapped in a div")
  h.assert_eq(out[2].classes[1], "term-callout-list", "div class")
  h.assert_eq(out[2].classes[2], class, "counter style class")
  h.assert_eq(out[2].content[1].t, "OrderedList", "the list itself")
  h.assert_eq(#out[2].content[1].content, 2, "list items kept")
  h.assert_eq(out[3].t, "Para", "following block kept")
end

-- wrap: no list right after the marker: only the marker goes
do
  local out = cl.wrap(pandoc.Blocks({ output, cl.marker(symbols), pandoc.Para("text"), list(item("a")) }))
  h.assert_eq(#out, 3, "marker removed")
  h.assert_eq(out[2].t, "Para", "paragraph not touched")
  h.assert_eq(out[3].t, "OrderedList", "list after a paragraph not wrapped")
end

-- wrap: a bullet list isn't wrapped
do
  local out = cl.wrap(pandoc.Blocks({ output, cl.marker(symbols), pandoc.BulletList({ item("a") }) }))
  h.assert_eq(out[2].t, "BulletList", "bullet list not wrapped")
end

-- wrap: marker at the end of the blocks
do
  local out = cl.wrap(pandoc.Blocks({ output, cl.marker(symbols) }))
  h.assert_eq(#out, 1, "trailing marker removed")
end

-- wrap: nothing to do
do
  h.assert_nil(cl.wrap(pandoc.Blocks({ output, list(item("a")) })), "unchanged blocks: nil")
  h.assert_nil(cl.wrap(pandoc.Blocks({ pandoc.RawBlock("html", "<!-- other comment -->") })), "other comments: nil")
end

-- wrap: two cells in a row
do
  local out = cl.wrap(pandoc.Blocks({ output, cl.marker(symbols), list(item("a")), output, cl.marker(symbols), list(item("b")) }))
  h.assert_eq(#out, 4, "two outputs and two wrapped lists")
  h.assert_eq(out[2].t, "Div", "first list wrapped")
  h.assert_eq(out[4].t, "Div", "second list wrapped")
end

-- wrap through a document walk: nested in a list item
do
  local nested = list({ pandoc.Para("rule"), output, cl.marker(symbols), list(item("explanation")) })
  local doc = pandoc.Pandoc({ nested }):walk({ Blocks = cl.wrap })
  local outer = doc.blocks[1]
  h.assert_eq(outer.t, "OrderedList", "outer list not wrapped")
  local inner = outer.content[1]
  h.assert_eq(inner[3].t, "Div", "nested list wrapped")
  h.assert_eq(#inner, 3, "marker removed inside the list item")
end

h.finish("test_callout_list")
