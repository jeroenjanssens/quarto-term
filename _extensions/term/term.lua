local function find_engine()
  local release = "./target/release/quarto-term"
  local debug = "./target/debug/quarto-term"
  local f = io.open(release, "r")
  if f then
    f:close()
    return release
  end
  f = io.open(debug, "r")
  if f then
    f:close()
    return debug
  end
  return "quarto-term"
end

local ENGINE = find_engine()

local function parse_cell_options(text)
  local options = {}
  local code_lines = {}
  local in_options = true

  for line in text:gmatch("([^\n]*)\n?") do
    if in_options and line:match("^#|%s*") then
      local key, value = line:match("^#|%s*(%S+):%s*(.+)$")
      if key then
        value = value:gsub('^"(.*)"$', '%1')
        value = value:gsub("^'(.*)'$", '%1')
        options[key] = value
      end
    else
      in_options = false
      table.insert(code_lines, line)
    end
  end

  if #code_lines > 0 and code_lines[#code_lines] == "" then
    table.remove(code_lines)
  end

  return options, table.concat(code_lines, "\n")
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

local function build_cell(block)
  local cell_options, code = parse_cell_options(block.text)

  local options = {
    echo = true,
    output = true,
    reverse = false,
  }

  if cell_options["echo"] == "false" then options.echo = false end
  if cell_options["output"] == "false" then options.output = false end
  if cell_options["reverse"] == "true" then options.reverse = true end
  if cell_options["prefix"] then options.prefix = cell_options["prefix"] end

  if block.attributes["echo"] == "false" then options.echo = false end
  if block.attributes["output"] == "false" then options.output = false end
  if block.attributes["reverse"] == "true" then options.reverse = true end
  if block.attributes["prefix"] then options.prefix = block.attributes["prefix"] end

  return { code = code, options = options }
end

function Pandoc(doc)
  -- Collect all term blocks and their positions
  local term_indices = {}
  local cells = {}

  for i, block in ipairs(doc.blocks) do
    if is_term_block(block) then
      table.insert(term_indices, i)
      table.insert(cells, build_cell(block))
    end
  end

  if #cells == 0 then
    return nil
  end

  -- Send all cells to the Rust engine in a single batch
  local input = pandoc.json.encode(cells)
  local ok, result = pcall(pandoc.pipe, ENGINE, {}, input)
  if not ok then
    io.stderr:write("quarto-term engine error: " .. tostring(result) .. "\n")
    return nil
  end

  local results = pandoc.json.decode(result)

  -- Replace term blocks with the engine's HTML output
  for idx, block_index in ipairs(term_indices) do
    local cell_result = results[idx]
    if cell_result and cell_result.html and cell_result.html ~= "" then
      doc.blocks[block_index] = pandoc.RawBlock("html", cell_result.html)
    else
      doc.blocks[block_index] = pandoc.Null()
    end
  end

  return doc
end
