# Option Promotion Plan

## Goal

Promote options to additional levels for more flexible configuration, while preserving backwards compatibility and not changing the existing API behavior when no new options are set.

## Changes Overview

| Option | Current Levels | New Levels | Notes |
|--------|---------------|------------|-------|
| `delay` | cell, line | **doc**, cell, line | Doc default replaces hardcoded 0.1 |
| `hold` | cell, line | **doc**, cell, line | Doc default for post-cell hold |
| `echo` | cell | **doc**, cell | Doc default replaces hardcoded "terminal" |
| `keep-last-prompt` | cell | **doc**, cell | Doc default replaces hardcoded false |
| `highlight` | cell | **doc**, cell | Doc default replaces hardcoded "bash" |
| `remove` | cell | **doc**, cell | Doc patterns merged with cell patterns |
| `enter` | line | **cell**, line | Cell default used when line doesn't specify |
| `expect-prompt` | line | **cell**, line | Cell default used when line doesn't specify |
| `timeout` | doc, cell | doc, cell, **line** | Per-line timeout override |
| `typing` | doc, cell | doc, cell, **line** | Per-line bool toggle |
| `spacing` | doc, cell | under **style** block | Move canonical location to style; keep flat key as compat |

## Detailed Changes

### 1. `src/protocol.rs`

#### Config struct — add 6 new fields:

```rust
#[serde(default = "default_delay")]
pub delay: f64,                          // default 0.1

#[serde(default)]
pub hold: f64,                           // default 0.0

#[serde(default = "default_echo")]
pub echo: EchoMode,                      // default "terminal"

#[serde(default)]
pub keep_last_prompt: bool,              // default false

#[serde(default = "default_highlight")]
pub highlight: HighlightSpec,            // default "bash"

#[serde(default)]
pub remove: Vec<AnnotationSpec>,         // default []
```

Add `default_delay` function:
```rust
fn default_delay() -> f64 { 0.1 }
```

#### CellOptions struct — make 3 fields Optional:

- `echo: EchoMode` → `echo: Option<EchoMode>` (remove `default = "default_echo"`)
- `keep_last_prompt: bool` → `keep_last_prompt: Option<bool>`
- `highlight: HighlightSpec` → `highlight: Option<HighlightSpec>` (remove `default = "default_highlight"`)

#### CellOptions struct — add 2 new fields:

```rust
#[serde(default)]
pub enter: Option<bool>,

#[serde(default)]
pub expect_prompt: Option<bool>,
```

#### LineOptions struct — add 2 new fields:

```rust
#[serde(default)]
pub timeout: Option<f64>,

#[serde(default)]
pub typing: Option<bool>,
```

#### Display impl for CellOptions — update for Option types:

- `echo`: wrap match in `if let Some(echo) = &self.echo { ... }`
- `keep_last_prompt`: change `if self.keep_last_prompt` to `if self.keep_last_prompt == Some(true)`

### 2. `src/session.rs`

#### `execute_cell()` — delay fallback (line 185):

```rust
let cell_delay = cell.options.delay.unwrap_or(self.config.delay);
```

#### `execute_cell()` — timeout setup:

```rust
let cell_timeout = cell.options.timeout.unwrap_or(orig_timeout);
self.config.timeout = cell_timeout;
```

#### `execute_cell()` — enter/expect_prompt resolution:

```rust
let cell_enter = cell.options.enter.unwrap_or(literal);
let enter = line_opts.and_then(|lo| lo.enter).unwrap_or(cell_enter);
let cell_expect = cell.options.expect_prompt.unwrap_or(enter);
let expect_prompt = line_opts.and_then(|lo| lo.expect_prompt).unwrap_or(cell_expect);
```

#### `execute_cell()` — per-line timeout (inside loop):

```rust
self.config.timeout = line_opts.and_then(|lo| lo.timeout).unwrap_or(cell_timeout);
```

#### `execute_cell()` — per-line typing (inside loop):

```rust
let line_is_human = match line_opts.and_then(|lo| lo.typing) {
    Some(true) => true,
    Some(false) => false,
    None => is_human,
};
```

#### `execute_cell()` — post-cell hold:

```rust
let cell_hold = cell.options.hold.unwrap_or(self.config.hold);
if cell_hold > 0.0 {
    self.drain_during(Duration::from_secs_f64(cell_hold));
}
```

#### `build_cell_html()` — fallbacks:

```rust
let echo_mode = cell.options.echo.as_ref().unwrap_or(&self.config.echo);
let keep_last_prompt = cell.options.keep_last_prompt.unwrap_or(self.config.keep_last_prompt);
let highlight = cell.options.highlight.as_ref().unwrap_or(&self.config.highlight);

// remove: merge doc + cell
let mut combined_remove = self.config.remove.clone();
combined_remove.extend(cell.options.remove.iter().cloned());
apply_remove(&mut lines, &combined_remove);
```

### 3. `_extensions/term/term.lua`

#### `extract_config()` — read new doc-level options (after line 365):

```lua
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
if term_meta["remove"] ~= nil then
  -- Parse list of pattern strings for doc-level remove
  local rv = term_meta["remove"]
  if type(rv) == "table" and rv.t == nil and #rv > 0 then
    config.remove = {}
    for i = 1, #rv do table.insert(config.remove, meta_str(rv[i])) end
  else
    local s = meta_str(rv)
    if s then config.remove = {s} end
  end
end
```

#### `extract_style()` — add `spacing` read:

```lua
if style_meta["spacing"] ~= nil then s.spacing = meta_bool(style_meta["spacing"]) end
```

#### Style application — add `spacing`:

```lua
if style.spacing ~= nil then config.spacing = style.spacing end
```

#### `build_cell()` — make echo/highlight conditional:

Remove `echo = "terminal"` and `highlight = "bash"` from the initial options table. Only set them when explicitly present in cell_opts.

#### `build_cell()` — add enter/expect-prompt at cell level:

```lua
local enter_val = cell_opts["enter"]
if enter_val ~= nil then options.enter = enter_val end
local ep_val = cell_opts["expect-prompt"] or cell_opts["expect_prompt"]
if ep_val ~= nil then options.expect_prompt = ep_val end
```

#### `build_cell()` — add spacing to cell style:

```lua
if cell_style["spacing"] ~= nil then options.spacing = cell_style["spacing"] end
```

### 4. `docs/reference.qmd`

Update all option tables to show new levels.

## Resolution Chains

| Option | Line → | Cell → | Doc/Config → | Hard default |
|--------|--------|--------|-------------|--------------|
| `delay` | lo.delay | cell.delay | config.delay | 0.1 |
| `hold` (per-line) | lo.hold | — | — | 0.1 (hardcoded capture window) |
| `hold` (post-cell) | — | cell.hold | config.hold | 0.0 |
| `enter` | lo.enter | cell.enter | — | resolved `literal` |
| `expect-prompt` | lo.expect_prompt | cell.expect_prompt | — | resolved `enter` |
| `timeout` | lo.timeout | cell.timeout | config.timeout | 10.0 |
| `typing` (enable) | lo.typing (bool) | cell.typing | config.typing | disabled |
| `echo` | — | cell.echo | config.echo | "terminal" |
| `keep-last-prompt` | — | cell.keep_last_prompt | config.keep_last_prompt | false |
| `highlight` | — | cell.highlight | config.highlight | "bash" |
| `spacing` | — | cell.spacing / style.spacing | config.spacing / style.spacing | false |
| `remove` | — | config.remove + cell.remove | config.remove | [] |

## Implementation Order

1. `src/protocol.rs` — struct changes
2. `src/session.rs` — resolution logic
3. `_extensions/term/term.lua` — Lua filter
4. `cargo test` + e2e verification
5. `docs/reference.qmd` — documentation

## TODO

- [ ] protocol.rs: Add Config fields (delay, hold, echo, keep_last_prompt, highlight, remove)
- [ ] protocol.rs: Make CellOptions echo/keep_last_prompt/highlight Optional
- [ ] protocol.rs: Add CellOptions enter/expect_prompt
- [ ] protocol.rs: Add LineOptions timeout/typing
- [ ] protocol.rs: Update Display impl
- [ ] protocol.rs: Add default_delay function + tests
- [ ] session.rs: Delay fallback → config.delay
- [ ] session.rs: Enter/expect_prompt → cell-level defaults
- [ ] session.rs: Per-line timeout resolution
- [ ] session.rs: Per-line typing resolution
- [ ] session.rs: Post-cell hold → config.hold
- [ ] session.rs: Echo/keep_last_prompt/highlight → config fallbacks
- [ ] session.rs: Remove → merge config + cell
- [ ] term.lua: Read doc-level delay/hold/echo/keep-last-prompt/highlight/remove
- [ ] term.lua: Add spacing to style extraction + application
- [ ] term.lua: Make echo/highlight conditional in build_cell
- [ ] term.lua: Add enter/expect-prompt cell reads
- [ ] term.lua: Add spacing to cell style block
- [ ] Verify all tests pass
- [ ] docs/reference.qmd: Update option tables
