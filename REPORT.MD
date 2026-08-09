# Roborev Report: quarto-term (Jobs 144–151)

Compiled 2026-08-08. Eight automated reviews by Copilot covering refactoring, complexity, API design, dead code, architecture, security, and duplication.

---

## Phase 1: Critical Fixes

Quick wins that fix bugs, security issues, and dead code. Each task is independent and can be done in any order.

---

### 1.1 Fix Typst raw-block injection

**Why:** `src/typst.rs:96-110` wraps terminal output in a fixed triple-backtick fence. Command output containing triple backticks can close the raw block and inject arbitrary Typst code (e.g., `#read("...")`), leaking local files during PDF rendering. This is exploitable whenever untrusted command output is rendered to Typst/PDF.

**How:** Use a dynamic fence — scan the content for the longest backtick run, then use a fence one longer. Alternatively, escape backtick sequences within the content.

**TODO:**
- [ ] In `src/typst.rs`, find the longest consecutive backtick run in the rendered content
- [ ] Use a fence of length `max_run + 1` (minimum 3)
- [ ] Add a test in `src/typst.rs` tests: feed a line containing ` ``` ` and `#read("/etc/passwd")` — verify they appear as literal text, not Typst directives
- [ ] Add a test with content containing backtick runs of varying lengths (4, 5, 10)

---

### 1.2 Fix `cache: false` being broken

**Why:** In `term.lua`, `config._cache` is cleared before the line `local use_cache = config._cache ~= false`, so `use_cache` is always `true`. The documented `cache: false` option does nothing. Users who set it expect cache bypass (e.g., during development/debugging).

**How:** Read the cache setting before clearing config internals, or restructure so the cache flag is preserved through normalization.

**TODO:**
- [ ] In `term.lua`, locate where `config._cache` is cleared
- [ ] Move the `use_cache` assignment to before the clearing, or preserve the value through a local
- [ ] Add a Lua test: `extract_config` with `cache: false` → verify the flag is preserved and passed through
- [ ] Verify manually: set `cache: false` in a doc, change command output, re-render → confirm no stale cache

---

### 1.3 Remove dead variables in term.lua

**Why:** `cache_hit` is assigned but never read. `nested_key` is declared but never used. Dead code adds noise and confuses future readers.

**How:** Delete the assignments.

**TODO:**
- [ ] Remove `cache_hit` assignment (search for where it's set)
- [ ] Remove `nested_key` declaration
- [ ] Run Lua tests to confirm no breakage

---

### 1.4 Fix cell-level `colorscheme-light` / `colorscheme-dark`

**Why:** These options are parsed and stored at cell level but never applied during rendering. The docs claim cell-level support. Users trying to use per-cell light/dark schemes get silently ignored.

**How:** In the HTML rendering path where document-level colorscheme-light/dark is applied (CSS class or inline style injection), extend the logic to also check cell-level values.

**TODO:**
- [ ] Find where document-level `colorscheme-light`/`colorscheme-dark` produce output (likely the CSS variable/class injection in `term.lua`)
- [ ] Replicate that logic for per-cell values — add scoped CSS or inline style on the cell's container div
- [ ] Add a test: two cells in one doc, one with `colorscheme-light: github-light`, one with `colorscheme-light: solarized-light` — verify different classes/styles
- [ ] Update `docs/styling.qmd` with a cell-level light/dark example if not already present

---

### 1.5 Remove unused `TermError::Io`

**Why:** `src/error.rs` defines `TermError::Io` and `impl From<io::Error>` with no call sites. Dead code.

**How:** Delete the variant and the `From` impl.

**TODO:**
- [ ] Remove `Io(std::io::Error)` from the `TermError` enum
- [ ] Remove `impl From<io::Error> for TermError`
- [ ] Run `cargo build` to confirm no compile errors
- [ ] Run `cargo test` to confirm no test failures

---

### 1.6 Fix `typing: true` boolean inconsistency

**Why:** In Rust JSON protocol, `typing: true` (boolean) is treated as disabled/default. In Lua cell-level config, `typing: true` becomes `{}` and enables typing with defaults. This means the same logical intent produces opposite behavior depending on where it's specified.

**How:** Align both sides. `typing: true` should mean "enable typing with default speed/error-rate" everywhere. In Rust, treat `true` as `TypingConfig::default()` rather than ignoring it.

**TODO:**
- [ ] In `src/protocol.rs`, find where `typing` is deserialized — handle `Value::Bool(true)` as `Some(TypingConfig::default())`
- [ ] In `term.lua`, verify that `typing: true` produces `typing: {}` or `typing: {"speed": ..., "error_rate": ...}` with defaults
- [ ] Add a Rust test: JSON with `"typing": true` → verify typing is enabled with default config
- [ ] Add a Rust test: JSON with `"typing": false` → verify typing is disabled
- [ ] Add a Rust test: JSON with `"typing": {"speed": 50}` → verify custom config

---

## Phase 2: API Consistency & Validation

Fixes that improve the user experience by catching errors early and making the option surface coherent.

---

### 2.1 Validate unknown option values

**Why:** Unknown enum values for `echo`, `format`, `pull`, and colorscheme names silently no-op. A user who typos `colorscheme: dracual` gets no feedback — just default styling. This wastes debugging time.

**How:** Add validation at the Lua level (before sending to Rust) and at the Rust protocol deserialization level. Error hard on invalid values — no silent fallbacks.

**TODO:**
- [ ] In `term.lua`, after resolving colorscheme name, check if the CSS file exists — warn if not
- [ ] In `term.lua`, validate `echo` values against known set (`true`, `false`, `"source"`, `"fenced"`)
- [ ] In `src/protocol.rs`, validate `format` against known values during deserialization — return error for unknowns
- [ ] In `src/protocol.rs`, validate `pull` against `"always"`, `"missing"`, `"never"` — return error for unknowns
- [ ] Add Lua test: unknown colorscheme name → warning logged
- [ ] Add Rust test: unknown format string → error returned

---

### 2.2 Propagate recorder errors

**Why:** `src/session.rs` uses `.ok()` and `let _ = ...` for recorder creation and writes. If a user requests `record: output.cast` and the path is unwritable, they get no feedback — just a missing file.

**How:** Return errors from recorder creation. For write errors during session, at minimum log a warning (recording is non-critical to execution).

**TODO:**
- [ ] In `src/session.rs`, change recorder creation from `.ok()` / `.filter_map()` to `.collect::<Result<Vec<_>, _>>()?`
- [ ] Surface a clear error message when a record path is unwritable
- [ ] For mid-session write errors, log to stderr rather than silently ignoring
- [ ] Add a test: request recording to an invalid path → verify error is returned
- [ ] Verify existing tests still pass (some may rely on silent failure)

---

### 2.3 Fix inconsistent naming

**Why:** Option naming drift across Lua/Rust/docs creates confusion. Examples: `fontsize` vs `font-size`, `theme` vs `colorscheme`, `disable-auto-indent` vs `auto-indent`.

**Canonical names:** `font-size` (not `fontsize`), `colorscheme` (not `theme`). The term "theme" should not appear in the user-facing API or codebase.

**How:** Rename to the canonical names everywhere. No aliases, no backwards compatibility — just remove the old names entirely.

**TODO:**
- [ ] Audit all option names in `src/protocol.rs`, `term.lua`, and docs
- [ ] Rename any uses of `theme` to `colorscheme` (structs, variables, fields, function names, file references)
- [ ] Rename any uses of `fontsize` to `font-size`
- [ ] Remove `disable-auto-indent` — use a single canonical name (pick one: `auto-indent: false` or `strip-auto-indent: true`)
- [ ] Update all docs to use canonical names only
- [ ] Remove all dead aliases — no backwards compatibility shims
- [ ] Add tests verifying the canonical names work and old names are not silently accepted

---

### 2.4 Add shell presets

**Why:** Configuring a non-bash shell requires setting `shell`, `shell-args`, `prompt`, and `ps2` manually every time. This is error-prone and verbose. Presets like `profile: python` would set all four in one line.

**How:** Define a preset table in Lua mapping profile names to their full config. When `profile` is set, merge preset values as defaults (user overrides still win).

**TODO:**
- [ ] Define presets in `term.lua` for: `python`, `node`, `r`, `ruby`, `sqlite`, `duckdb`, `fish`, `nushell`, `sh`
- [ ] Each preset sets: `shell`, `shell-args` (if needed), `prompt` regex, `ps2` regex, `quit` command
- [ ] In `extract_config` or `build_cell`, if `profile` is set, merge preset values under user-specified values
- [ ] Add Lua test: `profile: python` → verify shell is `python3`, prompt is `>>> `, etc.
- [ ] Add Lua test: `profile: python` + explicit `prompt: "py> "` → verify user override wins
- [ ] Update `docs/shells/index.qmd` to mention profiles as the easy path
- [ ] Add a `profiles` section to `docs/reference.qmd`

---

## Phase 3: Test Infrastructure

Reduce test boilerplate and make adding new tests cheap.

---

### 3.1 Create `tests/lua/helpers.lua`

**Why:** All 4 Lua test files repeat: pass/fail counters, `assert_eq`/`assert_true`/`assert_nil` helpers, `term.lua` path discovery, source loading, function extraction via string patterns, `mock_block`, and final report/exit logic. This makes new tests expensive to write and fragile to maintain.

**How:** Extract shared utilities into a single `helpers.lua` module that each test file requires.

**TODO:**
- [ ] Create `tests/lua/helpers.lua` with:
  - [ ] `assert_eq(actual, expected, msg)`
  - [ ] `assert_true(value, msg)`
  - [ ] `assert_nil(value, msg)`
  - [ ] `assert_not_nil(value, msg)`
  - [ ] `pass_count`, `fail_count` tracking
  - [ ] `finish(test_name)` — prints summary, exits with code
  - [ ] `load_term_functions(names, opts)` — discovers term.lua, extracts named functions
  - [ ] `mock_block(text, classes)` — creates a mock Pandoc CodeBlock
  - [ ] `term_meta(opts)` — builds a metadata table for `extract_config`
- [ ] Refactor `test_build_cell.lua` to use helpers
- [ ] Refactor `test_coerce_value.lua` to use helpers
- [ ] Refactor `test_extract_config.lua` to use helpers
- [ ] Refactor `test_parse_cell_options.lua` to use helpers
- [ ] Run all Lua tests to confirm they still pass

---

### 3.2 Replace hand-built JSON in integration tests

**Why:** `tests/binary_protocol.rs` constructs JSON request strings by hand ~15 times. This is verbose, error-prone, and fragile when the protocol changes.

**How:** Use `serde_json::json!` macro and helper functions for common patterns.

**TODO:**
- [ ] Add helper functions at the top of `tests/binary_protocol.rs`:
  - [ ] `fn request(cells: Vec<Value>) -> Value` — wraps cells in a default config
  - [ ] `fn request_with_config(config: Value, cells: Vec<Value>) -> Value`
  - [ ] `fn cell(id: u32, code: &str) -> Value` — minimal cell
  - [ ] `fn cell_with_opts(id: u32, code: &str, opts: Value) -> Value`
  - [ ] `fn run_request(req: &Value) -> Value` — pipes JSON through the binary, parses output
  - [ ] `fn assert_cell_ok(result: &Value, index: usize) -> &str` — extract HTML from successful cell
- [ ] Convert each existing test to use helpers
- [ ] Run `cargo test` to confirm all pass
- [ ] Verify test output is still clear on failure (error messages should show which cell failed)

---

### 3.3 Add Rust test support module

**Why:** Multiple Rust source files repeat `avt::Vt::builder().size(...).feed_str(...)` for creating test terminal lines, `RenderedLine { html, text }` construction, and default style/theme fixtures.

**How:** Add a `src/test_support.rs` module (behind `#[cfg(test)]`) with shared builders.

**TODO:**
- [ ] Create `src/test_support.rs` with:
  - [ ] `pub fn vt_line(ansi: &str, cols: usize) -> avt::Line`
  - [ ] `pub fn rendered(text: &str) -> RenderedLine` — plain text, html = escaped text
  - [ ] `pub fn rendered_html(html: &str, text: &str) -> RenderedLine`
  - [ ] `pub fn rendered_lines(texts: &[&str]) -> Vec<RenderedLine>`
  - [ ] `pub fn default_html_style() -> HtmlStyle`
  - [ ] `pub fn default_latex_theme() -> LatexTheme`
  - [ ] `pub fn default_typst_theme() -> TypstTheme`
  - [ ] `pub fn numbered_lines(n: usize) -> Vec<RenderedLine>`
- [ ] Add `#[cfg(test)] pub mod test_support;` to `src/main.rs` (or `lib.rs` if split)
- [ ] Refactor `src/terminal_line.rs` tests to use `vt_line`
- [ ] Refactor `src/renderer.rs` tests to use helpers
- [ ] Refactor `src/latex.rs` tests to use helpers
- [ ] Refactor `src/typst.rs` tests to use helpers
- [ ] Refactor `src/markdown.rs` tests to use helpers
- [ ] Run `cargo test` — all pass

---

## Phase 4: Type Safety & Format Handling

Replace stringly-typed patterns with proper enums and traits.

---

### 4.1 Add `OutputFormat` enum in Rust

**Why:** Format strings `"html"`, `"latex"`, `"typst"`, `"markdown"` are matched across `protocol.rs`, `session.rs`, and all renderer modules. A typo compiles fine but produces wrong output silently. An enum catches this at compile time and makes adding/removing formats explicit.

**How:** Define `OutputFormat` enum, implement `Deserialize` (with error on unknown), and replace all string-based format matching.

**TODO:**
- [ ] Define in `src/protocol.rs`:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
  #[serde(rename_all = "lowercase")]
  enum OutputFormat { Html, Latex, Typst, Markdown }
  ```
- [ ] Change `BatchRequest.format` from `String` to `OutputFormat`
- [ ] Update `session.rs` format matching: replace string literals with enum variants
- [ ] Update `build_cell_html` (rename to `build_cell_output`) to dispatch on enum
- [ ] Update renderer selection logic
- [ ] Ensure unknown format in JSON → deserialization error with clear message
- [ ] Run `cargo test` — fix any compilation errors from the switch
- [ ] Update `term.lua` to validate format before sending (should already be lowercase string)

---

### 4.2 Extract shared renderer trait

**Why:** `renderer.rs` (HTML), `latex.rs`, `typst.rs`, and `markdown.rs` all iterate terminal lines, extract styled runs, escape text, and wrap with format-specific markup. The control flow is ~95% identical; only escaping and wrapping differ. When a rendering behavior needs to change (e.g., trailing space handling), it must be fixed in 4 places.

**How:** Define a `RenderBackend` trait with format-specific methods, then implement shared line-walking logic once.

**TODO:**
- [ ] Define trait in a new `src/render_backend.rs`:
  ```rust
  trait RenderBackend {
      fn escape(&self, text: &str) -> String;
      fn wrap_styled(&self, text: &str, pen: &Pen) -> String;
      fn wrap_line(&self, content: &str) -> String;
      fn wrap_block(&self, lines: &str, fullscreen: bool) -> String;
  }
  ```
- [ ] Implement for `HtmlBackend`, `LatexBackend`, `TypstBackend`, `MarkdownBackend`
- [ ] Implement shared `fn render_lines<B: RenderBackend>(lines: &[Line], backend: &B, ansi: bool, trailing: bool) -> Vec<RenderedLine>`
- [ ] Replace the body of `render_lines_to_html`, `render_lines_to_latex`, `render_lines_to_typst`, `render_lines_to_markdown` with calls to the shared function
- [ ] Keep the existing public API signatures intact (avoid breaking callers)
- [ ] Run `cargo test` — verify all renderer tests pass
- [ ] Manually check one rendered doc in each format to confirm output unchanged

---

### 4.3 Table-drive `keymap.rs`

**Why:** Key-name mapping is duplicated between `translate` (base keys) and `arrow_or_special_csi` (modified keys). Adding a new key requires editing two match arms. A shared lookup table removes this duplication (~80 LOC savings).

**How:** Define a static table of `(aliases, escape_sequence)` pairs, then look up by name in both functions.

**TODO:**
- [ ] Define `static NAMED_KEYS: &[(&[&str], &[u8])]` with entries like `(&["pageup", "page-up", "page_up"], b"\x1b[5~")`
- [ ] Refactor `translate()` to look up in `NAMED_KEYS` first, fall back to single-char logic
- [ ] Refactor `arrow_or_special_csi()` to find the key in `NAMED_KEYS` and apply the modifier CSI prefix
- [ ] Run existing keymap tests — all should pass unchanged
- [ ] Verify the `ctrl+home`, `shift+tab`, etc. modifier combinations still work

---

## Phase 5: Architecture — Split `session.rs`

The largest single-file maintainability problem. ~1700 lines mixing 7 concerns.

---

### 5.1 Extract `src/docker.rs`

**Why:** Docker availability detection, image pulling, and command construction are self-contained and unrelated to PTY session logic. Separating them makes the Docker path testable without a real PTY.

**How:** Move Docker-related functions and the Docker command builder into their own module.

**TODO:**
- [ ] Create `src/docker.rs`
- [ ] Move `check_docker_available()`, `pull_image()`, and Docker command construction from `PtySession::new`
- [ ] Define `pub fn docker_command(config: &BatchConfig) -> Command` (or similar)
- [ ] Keep `PtySession::new` calling into the new module
- [ ] Run `cargo test` — Docker-related tests still pass
- [ ] Optionally: add a unit test for `docker_command()` that verifies correct volume mounts and env vars without actually running Docker

---

### 5.2 Extract `src/typing.rs`

**Why:** The human-typing simulation (random delays, error injection, correction) is a self-contained feature that doesn't need access to PTY internals beyond "write bytes" and "sleep."

**How:** Move typing logic into its own module with a simple interface.

**TODO:**
- [ ] Create `src/typing.rs` (or rename existing if already partially there)
- [ ] Move typing simulation functions: character-by-character sending, delay calculation, error injection
- [ ] Define a public function like `pub fn type_line(writer: &mut dyn Write, text: &str, config: &TypingConfig)` or similar
- [ ] Update `session.rs` to call into `typing.rs`
- [ ] Run `cargo test`
- [ ] Add a unit test: verify typing produces correct byte sequence (ignoring timing)

---

### 5.3 Extract `src/output_capture.rs`

**Why:** Capturing terminal output — deciding which lines are new since the last prompt, handling fullscreen viewport capture vs. scrollback capture — is a distinct responsibility from executing commands.

**How:** Move capture logic into its own module.

**TODO:**
- [ ] Create `src/output_capture.rs`
- [ ] Move logic that determines which lines to capture (new-output mode vs. fullscreen `vt.view()`)
- [ ] Move scroll position tracking if it's only used for capture decisions
- [ ] Define a clear interface: given current terminal state + last prompt position → captured lines
- [ ] Update `session.rs` to use the new module
- [ ] Run `cargo test`

---

### 5.4 Extract `src/output_transform.rs`

**Why:** `remove`, `truncate`, `callouts`, and `spacing` transforms are applied post-capture and are independent of each other and of terminal/PTY state. They should be a pure function from `(lines, options) -> lines`.

**How:** Move all line-transform functions into their own module.

**TODO:**
- [ ] Create `src/output_transform.rs`
- [ ] Move `apply_remove()`, `apply_truncate()`, `apply_callouts()`, `apply_spacing()`
- [ ] Move `LineSpec` parsing (`resolve_line_specs`, range parsing, regex selectors)
- [ ] Define a pipeline interface:
  ```rust
  pub fn transform_lines(lines: &mut Vec<RenderedLine>, opts: &TransformOpts, format: OutputFormat)
  ```
- [ ] Fix callouts to be format-aware (currently always emits HTML spans — should emit LaTeX/Typst equivalents too)
- [ ] Update `session.rs` to call the new module
- [ ] Move existing transform tests to the new module
- [ ] Run `cargo test`

---

### 5.5 Extract `src/prompt.rs`

**Why:** Prompt detection (primary prompt regex, PS2 regex, last-line-is-prompt checks, PS2 recovery) is complex logic that interacts with terminal state but doesn't need the full PTY session context.

**How:** Move prompt-related logic into its own module.

**TODO:**
- [ ] Create `src/prompt.rs`
- [ ] Move `last_line_is_primary_prompt()`, PS2 recovery logic, prompt regex construction
- [ ] Move `mark_prompts()` (wrapping lines with prompt/output spans)
- [ ] Define a `PromptDetector` struct that holds compiled regexes
- [ ] Update `session.rs` to use `PromptDetector`
- [ ] Run `cargo test`

---

### 5.6 Slim down `session.rs`

**Why:** After the above extractions, `session.rs` should only contain PTY session lifecycle (new/drop), line sending, and the `execute_cell` orchestration that ties everything together. Target: under 500 lines.

**TODO:**
- [ ] Verify `session.rs` only contains: `PtySession` struct, `new()`, `execute_cell()`, `process_batch()`
- [ ] `execute_cell` should read as a high-level orchestration: resolve options → send lines → capture → transform → render
- [ ] Remove the temporary `self.config.timeout` mutation (use a local instead)
- [ ] Run full test suite
- [ ] Verify line count is manageable (~400-600 lines)

---

## Phase 6: Architecture — Modularize `term.lua`

The Lua filter is ~1300 lines mixing 6 concerns. Quarto requires a single filter file, but we can `require()` local modules from the extension directory or use `dofile()`.

---

### 6.1 Extract `config.lua`

**Why:** `extract_config` is ~389 LOC of option parsing, coercion, default merging, and format-specific overrides. It's testable in isolation and changes independently from the rendering pipeline.

**How:** Move to `_extensions/term/config.lua` and `dofile()` it from `term.lua`.

**TODO:**
- [ ] Create `_extensions/term/config.lua`
- [ ] Move `extract_config`, `coerce_value`, `as_list`, and option normalization helpers
- [ ] Export via `return { extract_config = ..., coerce_value = ..., ... }`
- [ ] In `term.lua`, load with `local config_mod = dofile(PANDOC_SCRIPT_FILE:match("(.*/)")  .. "config.lua")`
- [ ] Verify Quarto still discovers and runs the filter correctly
- [ ] Update Lua tests to load from the new file (simpler than regex extraction)
- [ ] Run all Lua tests

---

### 6.2 Extract `cell.lua`

**Why:** `parse_cell_options` and `build_cell` handle per-cell option parsing and request construction. They're self-contained and heavily tested.

**How:** Move to `_extensions/term/cell.lua`.

**TODO:**
- [ ] Create `_extensions/term/cell.lua`
- [ ] Move `parse_cell_options`, `build_cell`, and any helper they use (e.g., `escape_pattern`)
- [ ] Export via return table
- [ ] Load from `term.lua`
- [ ] Update Lua tests to import directly
- [ ] Run all Lua tests

---

### 6.3 Extract `theme.lua`

**Why:** Theme/colorscheme resolution, CSS file reading, and light/dark mode handling are independent of the main filter pipeline.

**How:** Move to `_extensions/term/theme.lua`.

**TODO:**
- [ ] Create `_extensions/term/theme.lua`
- [ ] Move colorscheme resolution, CSS reading, theme variable rescoping
- [ ] Export via return table
- [ ] Load from `term.lua`
- [ ] Verify themes still apply correctly (render a doc with colorscheme)
- [ ] Run Lua tests

---

### 6.4 Extract `engine.lua`

**Why:** Cache management and `pandoc.pipe()` invocation are the I/O boundary of the filter. Separating them clarifies what's pure logic vs. side effects.

**How:** Move to `_extensions/term/engine.lua`.

**TODO:**
- [ ] Create `_extensions/term/engine.lua`
- [ ] Move cache read/write logic, hash computation, `pandoc.pipe` call
- [ ] Export: `run_engine(request_json, config) -> results`
- [ ] Load from `term.lua`
- [ ] Verify caching still works (render twice, confirm cache hit)
- [ ] Verify `cache: false` works (after Phase 1 fix)

---

### 6.5 Extract `chrome.lua`

**Why:** `normalize_chrome()` and `wrap_chrome()` are self-contained HTML generation for window decorations. They're already well-isolated logically.

**How:** Move to `_extensions/term/chrome.lua`.

**TODO:**
- [ ] Create `_extensions/term/chrome.lua`
- [ ] Move `normalize_chrome`, `wrap_chrome`
- [ ] Export via return table
- [ ] Load from `term.lua`
- [ ] Verify chrome still renders for all three OS styles
- [ ] Add a Lua test for `normalize_chrome` (currently untested)

---

### 6.6 Slim down `term.lua`

**Why:** After extractions, `term.lua` should be a thin orchestrator: load modules, register the Pandoc filter, collect blocks, call engine, inject results and dependencies.

**TODO:**
- [ ] Verify `term.lua` is under ~200 lines
- [ ] It should read as: load modules → define `Pandoc` filter → collect term blocks → build request → call engine → inject results → inject CSS/JS
- [ ] Run full Quarto render of the docs site
- [ ] Run all Lua tests
- [ ] Run E2E tests

---

## Phase 7: Shared Config & Docs Generation

Eliminate config drift between Lua and Rust, and reduce manual maintenance of repetitive docs.

---

### 7.1 Create a shared config schema

**Why:** Options are defined independently in Lua (`extract_config`, `build_cell`) and Rust (`protocol.rs` structs). Defaults and validation can drift silently. A single source of truth prevents this.

**How:** Define options in a TOML/JSON schema file. Generate (or validate against) both Lua normalization and Rust serde structs. Alternatively: use the Rust structs as canonical and add conformance tests that verify Lua output matches.

**TODO:**
- [ ] Create `schema/options.toml` (or `.json`) defining every option: name, type, default, valid values, scope (project/doc/cell), aliases
- [ ] Write a script (or test) that reads the schema and verifies:
  - [ ] Every option in the schema has a corresponding field in `protocol.rs`
  - [ ] Every option in the schema is handled in `term.lua` `extract_config` or `build_cell`
  - [ ] Defaults match between Lua and Rust
- [ ] Alternatively: add integration tests that send known JSON to the binary and verify the defaults applied match what Lua would send
- [ ] Document the schema format for contributors

---

### 7.2 Generate docs reference table from schema

**Why:** `docs/reference.qmd` has a manually maintained option table that can drift from implementation. The colorscheme gallery in `docs/styling.qmd` manually lists every theme CSS file.

**How:** Write a small script that generates the reference table and colorscheme gallery from the schema and the `themes/` directory listing.

**TODO:**
- [ ] Write `scripts/generate-reference.lua` (or Python/shell) that:
  - [ ] Reads `schema/options.toml`
  - [ ] Outputs the Markdown table for `docs/reference.qmd`
  - [ ] Reads `_extensions/term/themes/*.css`
  - [ ] Outputs the colorscheme gallery sections for `docs/styling.qmd`
- [ ] Add a CI check: generated output matches committed docs (or make docs build auto-generate)
- [ ] Decide: check in generated docs (easier workflow) or generate at build time (always fresh)
- [ ] Update contributing guide to mention the generation step

---

### 7.3 Centralize line-selector parsing

**Why:** `remove`, `truncate`, and `callouts` each re-interpret `LineSpec` (ranges, regexes, single indices) with slightly different behavior. Adding a new selector type requires touching 3+ places.

**How:** Define a `LineSelector` abstraction that `remove`, `truncate`, and `callouts` all use.

**TODO:**
- [ ] Define `LineSelector` enum: `Index(i32)`, `Range(i32, i32)`, `Regex(String)`, `All`
- [ ] Implement `resolve(selector, lines) -> Vec<usize>` — returns matching line indices
- [ ] Refactor `apply_remove` to use `LineSelector::resolve`
- [ ] Refactor `apply_truncate` to use `LineSelector::resolve`
- [ ] Refactor `apply_callouts` to use `LineSelector::resolve`
- [ ] Add tests for edge cases: negative indices, out-of-range, regex matching multiple lines
- [ ] Move into `src/output_transform.rs` (or `src/line_selector.rs` if extracted separately)

---

## Phase 8: Miscellaneous Cleanup

Lower-priority items that round out the codebase quality.

---

### 8.1 Remove generated docs from git

**Why:** `docs/*_files/libs/` and `docs/_site/` contain ~3 MiB of duplicated Quarto build output (Bootstrap, reveal.js, etc.). These are generated artifacts that bloat the repo and create noisy diffs.

**How:** Add to `.gitignore`, remove from git history (or just from tracking going forward), and build via CI for deployment.

**TODO:**
- [ ] Add `docs/_site/` to `.gitignore`
- [ ] Add `docs/*_files/` to `.gitignore`
- [ ] `git rm -r --cached docs/_site/ docs/*_files/` (remove from tracking without deleting locally)
- [ ] Verify docs build still works: `cd docs && quarto render`
- [ ] Set up CI/GitHub Pages deployment if not already present
- [ ] Update README/contributing guide if it references committed site output

---

### 8.2 CLI parser for `quarto-term check`

**Why:** `src/main.rs` manually parses `--record` with custom arg iteration. Adding future flags (e.g., `--format`, `--verbose`) will make this increasingly fragile.

**How:** Use `clap` (already common in Rust) or keep manual parsing but wrap it in a clean function with proper error messages.

**TODO:**
- [ ] Evaluate: is `clap` worth adding as a dependency for 2-3 flags? (probably yes for correctness; adds ~200KB to binary)
- [ ] If yes: add `clap` to `Cargo.toml`, define a `Cli` struct with subcommands
- [ ] If no: at minimum extract arg parsing into a `fn parse_args(args: &[String]) -> Result<CliCommand, String>`
- [ ] Support `--help` with usage text
- [ ] Add a test: verify `quarto-term --help` exits 0 and prints usage
- [ ] Add a test: verify unknown flag produces helpful error

---

### 8.3 Consolidate shell E2E test runners

**Why:** `tests/e2e/run_e2e.sh` duplicates the Pandoc invocation pattern, pass/fail reporting, and Docker variant logic. Adding a new test case requires copying ~20 lines of boilerplate.

**How:** Extract shared functions and make test cases data-driven.

**TODO:**
- [ ] Create a `run_test()` function that takes: metadata YAML, input content, expected needle, assertion type (present/absent)
- [ ] Create a `run_docker_test()` wrapper that sets up Docker-specific env
- [ ] Convert all existing test cases to call the shared functions
- [ ] Add a summary at the end: N passed, M failed, with exit code
- [ ] Run the E2E suite to confirm all still pass

---

## Priority & Sequencing

| Phase | Effort | Risk | Dependency |
|-------|--------|------|-----------|
| 1. Critical Fixes | Low | Low | None — independent tasks |
| 2. API Consistency | Medium | Low | 1.6 depends on understanding protocol.rs (Phase 1 work helps) |
| 3. Test Infrastructure | Medium | Low | Do before Phase 5/6 to have good coverage during refactors |
| 4. Type Safety | Medium | Medium | OutputFormat (4.1) should happen before session.rs split (Phase 5) |
| 5. Split session.rs | High | Medium | Phase 3 + 4 first (tests + types catch regressions) |
| 6. Modularize term.lua | Medium | Medium | Phase 3.1 first (Lua test helpers) |
| 7. Schema & Docs Gen | Medium | Low | Phase 2.3 (naming) and Phase 6 (modular Lua) make this easier |
| 8. Miscellaneous | Low | Low | Independent — do whenever |

**Recommended order:** 1 → 3 → 4 → 2 → 5 → 6 → 7 → 8
