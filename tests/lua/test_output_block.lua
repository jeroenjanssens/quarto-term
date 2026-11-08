-- Test output.lua: the block for a cell's rendered output
-- (uses the real pandoc module, so no stubs)
local h = dofile(debug.getinfo(1, "S").source:sub(2):match("(.*[/\\])") .. "helpers.lua")
local output = dofile(h.project_root() .. "_extensions/term/output.lua")

local function fenced(lines, lang)
  return "```" .. (lang or "console") .. "\n" .. table.concat(lines, "\n") .. "\n```\n"
end

-- HTML, LaTeX, and Typst: always a raw block, as is (the binary escapes them)
for _, fmt in ipairs({"html", "latex", "typst"}) do
  local content = "x &#123;&#123;&lt; y"
  local block = output.block(fmt, content)
  h.assert_eq(block.t, "RawBlock", fmt .. ": raw block")
  h.assert_eq(block.format, fmt, fmt .. ": raw format")
  h.assert_eq(block.text, content, fmt .. ": content unchanged")
end

-- Markdown without `{{<`: a raw block, as is
do
  local content = fenced({"$ echo '{{x}} <y>'", "{{x}} <y>"})
  local block = output.block("markdown", content)
  h.assert_eq(block.t, "RawBlock", "markdown without {{<: raw block")
  h.assert_eq(block.text, content, "markdown without {{<: content unchanged")
end

-- Markdown with `{{<`: a code block with shortcodes="false"
do
  local lines = {"$ echo '{{< meta title >}}'", "{{< meta title >}}"}
  local block = output.block("markdown", fenced(lines))
  h.assert_eq(block.t, "CodeBlock", "markdown with {{<: code block")
  h.assert_eq(block.text, table.concat(lines, "\n"), "markdown with {{<: text kept")
  h.assert_eq(block.classes[1], "console", "markdown with {{<: class kept")
  h.assert_eq(block.attributes["shortcodes"], "false", "markdown with {{<: shortcodes off")
end

-- An unclosed `{{<` (the case that drops the rest of the page)
do
  local block = output.block("markdown", fenced({"alice{{<2,3}}.txt", "after"}))
  h.assert_eq(block.t, "CodeBlock", "unclosed {{<: code block")
  h.assert_eq(block.text, "alice{{<2,3}}.txt\nafter", "unclosed {{<: text kept")
end

-- Fullscreen output uses a `text` fence
do
  local block = output.block("markdown", fenced({"{{< x"}, "text"))
  h.assert_eq(block.classes[1], "text", "fullscreen: class text")
  h.assert_eq(block.attributes["shortcodes"], "false", "fullscreen: shortcodes off")
end

-- Longer fences (output with backticks) and blank lines and trailing spaces
do
  local content = "````console\n``` {{< x >}}\n\n  indented  \n````\n"
  local block = output.block("markdown", content)
  h.assert_eq(block.t, "CodeBlock", "long fence: code block")
  h.assert_eq(block.text, "``` {{< x >}}\n\n  indented  ", "long fence: text kept exactly")
end

-- Markdown that isn't a single code block stays a raw block
do
  local content = "{{< x >}}\n\nnot a code block\n"
  local block = output.block("markdown", content)
  h.assert_eq(block.t, "RawBlock", "not a code block: raw block")
end

h.finish("test_output_block")
