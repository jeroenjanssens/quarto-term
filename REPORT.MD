# Roborev Report: quarto-term (Jobs 152–159)

Compiled 2026-08-09. Eight automated reviews by Copilot covering test-fixtures, duplication, refactor, complexity, api-design, dead-code, architecture, and security.

---

## Phase 1: Critical Fixes

Security and correctness issues that affect users now. Each task is independent.

---

### 1.1 Fix Typst escape injection (backslash)

**Why:** `typst_escape()` in `src/typst.rs:155-169` escapes `#`, `[`, and `]` but not backslashes. Attacker-controlled terminal output containing `\] \#read("Cargo.toml") \[` can break out of the text block and cause Typst to evaluate arbitrary code during PDF rendering. This is exploitable whenever untrusted command output (logs, remote CLI output) is rendered to Typst/PDF.

**How:** Escape backslashes in `typst_escape()` before escaping other metacharacters. Add regression tests with ANSI-styled output containing escape sequences.

**TODO:**
- [ ] In `src/typst.rs`, add backslash escaping to `typst_escape()` — escape `\` before `#`, `[`, `]`
- [ ] Add test: feed a line containing `\] \#read("/etc/passwd") \[` — verify it appears as literal text
- [ ] Add test: ANSI-styled (colored) output containing backslash sequences — verify no Typst evaluation
- [ ] Add test: verify existing content with legitimate backslashes (file paths) renders correctly
- [ ] Run `cargo test`

---

### 1.2 Fix stale HTML dependency version strings

**Why:** Hard-coded `0.2.0` in `term.lua:1123,1143` while the package is at `0.4.4`, producing stale asset paths like `quarto-term-0.2.0/term.css`. This means users loading the extension at version 0.4.4 get CSS paths that don't match expectations and could conflict with older cached versions.

**How:** Read the version from `_extension.yml` or define a single `VERSION` constant at the top of `term.lua` and reference it in all dependency paths.

**TODO:**
- [ ] Define a `VERSION` constant near the top of `term.lua` (or read from `_extension.yml`)
- [ ] Replace hard-coded `"0.2.0"` at line ~1123 with the constant
- [ ] Replace hard-coded `"0.2.0"` at line ~1143 with the constant
- [ ] Search for any other hard-coded version strings in `term.lua`
- [ ] Render a doc and verify the generated `site_libs` path uses the correct version
- [ ] Run Lua tests

---

### 1.3 Rename `theme_bg`/`theme_fg` to `colorscheme_bg`/`colorscheme_fg`

**Why:** Project policy: never use "theme" in user-facing API. These internal variable names leak into debug output, error messages, and code review. Aligning them with the canonical `colorscheme` naming prevents accidental exposure and reduces cognitive load.

**How:** Find-and-replace in both Lua and Rust. The fields are internal, so no protocol or user-facing change is needed.

**TODO:**
- [ ] In `term.lua`, rename all `theme_bg` → `colorscheme_bg`, `theme_fg` → `colorscheme_fg`
- [ ] In `term.lua`, rename any functions/locals containing `theme` (e.g., `read_theme` → `read_colorscheme`)
- [ ] In Rust (`src/protocol.rs`, `src/session.rs`, `src/renderer.rs`), rename any `theme_bg`/`theme_fg` fields
- [ ] Verify the extracted `theme.lua` module is renamed or its internal references updated
- [ ] Run `cargo test` and Lua tests
- [ ] Grep entire codebase for remaining `theme` references — rename or document why they stay

---

## Phase 2: Dead Code Removal

Remove code that serves no purpose. Each task is trivial and independent.

---

### 2.1 Remove dead `quit` field from shell presets

**Why:** Shell profiles define `quit` values in Lua (`term.lua:187-195`), and these are sent in the JSON config to Rust. But `Config` in Rust has no `quit` field — Serde silently drops it. The field misleads contributors into thinking it's used.

**How:** Either remove `quit` from all shell presets in Lua, or implement graceful session teardown using the quit command. Removal is simpler and correct until quit behavior is needed.

**TODO:**
- [ ] In `term.lua`, locate the shell preset definitions (~line 187-195)
- [ ] Remove the `quit` key from every preset
- [ ] Search for any other references to `quit` in Lua (config extraction, build_cell)
- [ ] Remove any `quit`-related parsing/assignment
- [ ] Run Lua tests
- [ ] Run E2E tests to verify session teardown still works (it uses SIGHUP/close, not quit)

---

### 2.2 Remove `typing.mode` from Lua

**Why:** Lua copies `typing.mode` into config, but Rust's `TypingConfig` only uses `speed`/`error_rate`. The field is silently ignored, misleading users who set it.

**How:** Remove the `mode` parsing from Lua config extraction. If a future use is planned, add a comment explaining it's reserved.

**TODO:**
- [ ] In `term.lua`, locate where `typing.mode` is read/copied (around line 469)
- [ ] Remove the `mode` field handling
- [ ] If `mode` appears in docs (`reference.qmd`), remove it
- [ ] Run Lua tests
- [ ] Run `cargo test`

---

## Phase 3: API Consistency & Naming

Make the user-facing option surface coherent and well-validated.

---

### 3.1 Normalize option naming across layers

**Why:** Option naming drifts across Lua/Rust/docs. `reference.qmd` documents both `disable-auto-indent` and `auto-indent`, but implementation only reads `disable-auto-indent`. Kebab-case (YAML), snake_case (Rust), and mixed keys (Lua) coexist without clear rules.

**How:** Establish canonical kebab-case for all user-facing YAML options. Internally use snake_case in Rust and snake_case in Lua. Remove all aliases — breaking changes are fine.

**TODO:**
- [ ] Audit every option in `src/protocol.rs` — list current field names
- [ ] Audit every option in `term.lua` `extract_config` and `build_cell` — list current field names
- [ ] Audit `docs/reference.qmd` — list documented options
- [ ] Establish canonical name for each: `font-size` (not `fontsize`), `colorscheme` (not `theme`), etc.
- [ ] For `auto-indent`: pick one canonical form (recommend `auto-indent: true/false`, remove `disable-auto-indent`)
- [ ] Update `src/protocol.rs` serde attributes to use canonical kebab-case names
- [ ] Update `term.lua` to use canonical names in extraction/building
- [ ] Update all docs to use canonical names only
- [ ] Remove all dead aliases — no backwards compatibility
- [ ] Run full test suite
- [ ] Render docs site and verify no broken option references

---

### 3.2 Validate unknown option values

**Why:** Unknown enum values for `echo`, `format`, `pull`, and colorscheme names silently no-op. A user who typos `colorscheme: dracual` gets no feedback — just default styling. This wastes debugging time.

**How:** Add validation in Lua (before sending to Rust) and in Rust deserialization. Error on invalid values rather than silently falling back.

**TODO:**
- [ ] In `term.lua`, after resolving colorscheme name, check if the CSS file exists — emit `quarto.log.warning()` if not
- [ ] In `term.lua`, validate `echo` values against known set: `true`, `false`, `"source"`, `"fenced"`, `"terminal"`
- [ ] In `src/protocol.rs`, make `format` a strict enum (`Html`, `Latex`, `Typst`, `Markdown`) — unknown values → deserialization error
- [ ] In `src/protocol.rs`, validate `pull` against `"always"`, `"missing"`, `"never"` — error on unknown
- [ ] Add Lua test: unknown colorscheme name → warning logged
- [ ] Add Rust test: unknown format string → clear error message
- [ ] Add Rust test: unknown pull value → clear error message
- [ ] Run full test suite

---

### 3.3 Document display control interactions

**Why:** `echo`, `output`, `include`, and `eval` interact in surprising ways. Default `echo: terminal` shows output even with `output: false`. Users get confused when combinations don't do what they expect.

**How:** Add a truth table to the docs showing the behavior of each combination. This follows Quarto's own semantics, so we document rather than redesign.

**TODO:**
- [ ] In `docs/reference.qmd`, add a "Display Controls" section
- [ ] Create a truth table showing: `eval` × `echo` × `output` × `include` → what the user sees
- [ ] Note that `echo: terminal` overrides `output: false` behavior (this is by design)
- [ ] Add 2-3 examples showing common combinations: "show only output", "show only source", "hide everything but execute"
- [ ] Render and verify the docs page looks correct

---

### 3.4 Add diagnostics/warnings envelope to protocol

**Why:** The current protocol returns a raw result array. Warnings for unknown options, missing colorschemes, deprecated aliases, and non-fatal issues have nowhere to go except stderr (invisible to Lua). A structured diagnostics channel lets Lua surface them to the user.

**How:** Wrap the result array in `{ results, diagnostics }`. Each diagnostic has a severity, message, and optional cell reference.

**TODO:**
- [ ] In `src/protocol.rs`, define `BatchResponse { results: Vec<CellResult>, diagnostics: Vec<Diagnostic> }`
- [ ] Define `Diagnostic { severity: DiagnosticLevel, message: String, cell_id: Option<u32> }`
- [ ] Change `main.rs` to serialize `BatchResponse` instead of raw `Vec<CellResult>`
- [ ] Update `term.lua` JSON parsing to handle both old (array) and new (envelope) response formats during migration
- [ ] Add diagnostics for: unknown config keys, invalid colorscheme names, recorder path errors
- [ ] In `term.lua`, surface diagnostics as `quarto.log.warning()` calls
- [ ] Update `tests/binary_protocol.rs` to expect the new envelope format
- [ ] Run full test suite

---

## Phase 4: Code Structure — Rust

Decompose the two largest Rust files into focused modules.

---

### 4.1 Decompose `PtySession::execute_cell`

**Why:** 208 lines with cognitive complexity 32/25 (Clippy). Combines option resolution, typing/key sending, assertions, prompt recovery, timeout mutation, and rendering setup. The reader must track multiple state variables simultaneously: `last_enter`, `error`, `output_since_cell_start`, line overrides, prompt state, scrollback, and config overrides.

**How:** Extract logical phases into named methods. Keep the top-level `execute_cell` as a 30-line orchestrator calling into focused helpers.

**TODO:**
- [ ] Extract `fn resolve_line_options(&self, cell: &Cell) -> Vec<LineOptions>` — parses per-line overrides
- [ ] Extract `fn send_cell_line(&mut self, line: &str, opts: &LineOptions) -> Result<(), TermError>` — handles typing, key sending, delay
- [ ] Extract `fn check_assertions(&self, cell: &Cell, output: &[RenderedLine]) -> Vec<AssertionResult>` — pure logic
- [ ] Extract `fn recover_primary_prompt(&mut self) -> Result<(), TermError>` — PS2 recovery, timeout
- [ ] Replace timeout mutation with a local variable or scoped guard pattern
- [ ] Rewrite `execute_cell` as orchestrator: resolve → send lines → wait → capture → assert → render
- [ ] Run `cargo test` after each extraction step
- [ ] Verify E2E tests pass (prompt timing is sensitive)
- [ ] Check Clippy cognitive complexity is now under 25

---

### 4.2 Decompose `build_cell_html`

**Why:** 129 lines mixing echo handling, capture mode, output transforms, theme/style resolution, and format dispatch. Named `build_cell_html` but returns LaTeX/Typst/Markdown too — the name is actively misleading.

**How:** Rename to `build_cell_output`. Split into: source echo rendering, output capture, transform application, and format-specific rendering dispatch.

**TODO:**
- [ ] Rename `build_cell_html` → `build_cell_output`
- [ ] Extract `fn render_source_echo(&self, cell: &Cell, format: OutputFormat) -> Option<String>` — source/fenced echo modes
- [ ] Extract `fn apply_output_transforms(&self, lines: &mut Vec<RenderedLine>, cell: &Cell)` — remove, truncate, callouts, spacing
- [ ] Extract `fn render_output_block(&self, lines: &[RenderedLine], cell: &Cell, format: OutputFormat) -> String` — format dispatch
- [ ] Rewrite `build_cell_output` as: echo + capture + transform + render
- [ ] Update all call sites (should be just `execute_cell`)
- [ ] Run `cargo test`
- [ ] Render a doc in each format (HTML, LaTeX, Typst, Markdown) to verify output unchanged

---

### 4.3 Rename `RenderedLine.html` and `CellResult.html`

**Why:** These fields carry LaTeX/Typst/Markdown markup depending on format. The name `html` is misleading and causes confusion when working on non-HTML renderers. Every new contributor asks "why is this called html when it contains LaTeX?"

**How:** Rename to `markup` throughout. This is an internal-only field (JSON protocol between Lua and Rust), so no user-facing change.

**TODO:**
- [ ] In `src/terminal_line.rs` or wherever `RenderedLine` is defined, rename `html` → `markup`
- [ ] In `src/protocol.rs`, rename `CellResult.html` → `CellResult.markup`
- [ ] Update all Rust code referencing `.html` on these structs
- [ ] In `term.lua`, update JSON result parsing to read `markup` instead of `html`
- [ ] Add `#[serde(alias = "html")]` temporarily if needed for migration (remove once Lua is updated)
- [ ] Run `cargo test`
- [ ] Run Lua tests
- [ ] Run E2E tests
- [ ] Remove any temporary aliases

---

### 4.4 Introduce `LineSelector` enum

**Why:** `apply_remove`, `apply_truncate`, `apply_callouts`, and `resolve_line_specs` all independently parse index/range/regex patterns with slightly different behavior. Adding a new selector type requires touching 3+ places. Bugs in parsing are duplicated rather than fixed once.

**How:** Define a `LineSelector` enum that all line-targeting operations share. Parse once, resolve against any line set.

**TODO:**
- [ ] Define in `src/output_transform.rs`:
  ```rust
  enum LineSelector {
      Index(i32),          // positive or negative (from end)
      Range(i32, i32),     // inclusive
      Regex(String),       // matched against line text
      All,
  }
  ```
- [ ] Implement `fn parse_line_selector(spec: &str) -> Result<LineSelector, String>`
- [ ] Implement `fn resolve_indices(selector: &LineSelector, lines: &[RenderedLine]) -> Vec<usize>`
- [ ] Refactor `apply_remove` to use `parse_line_selector` + `resolve_indices`
- [ ] Refactor `apply_truncate` to use `parse_line_selector` + `resolve_indices`
- [ ] Refactor `apply_callouts` to use `parse_line_selector` + `resolve_indices`
- [ ] Add tests: negative indices, out-of-range, regex matching multiple lines, empty input
- [ ] Verify existing transform tests still pass
- [ ] Run `cargo test`

---

### 4.5 Fix default shell mismatch

**Why:** Lua/docs default to `zsh`; Rust `protocol.rs` defaults to `bash`. While Lua always sends an explicit shell value (making the Rust default unreachable), if the protocol is ever used directly or the Lua default changes, behavior would silently differ.

**How:** Change the Rust default to match Lua (`zsh`), or remove the Rust default entirely (require the field).

**TODO:**
- [ ] In `src/protocol.rs`, find the `shell` field default
- [ ] Change default from `"bash"` to `"zsh"` to match Lua/docs
- [ ] Alternatively: make `shell` a required field (remove `#[serde(default)]`)
- [ ] Run `cargo test` — fix any tests that relied on the old default
- [ ] Run `tests/binary_protocol.rs` — update any requests that omit `shell`

---

## Phase 5: Code Structure — Lua

Continue modularizing `term.lua` into focused modules loaded via `dofile()`.

---

### 5.1 Extract `config.lua`

**Why:** `extract_config` is ~405 lines of option parsing, coercion, default merging, and format-specific overrides. It's testable in isolation and changes independently from rendering. Extracting it eliminates the regex-based function extraction that Lua tests currently use.

**How:** Move to `_extensions/term/config.lua` and `dofile()` it from `term.lua`.

**TODO:**
- [ ] Create `_extensions/term/config.lua`
- [ ] Move `extract_config`, `coerce_value`, `as_list`, and all option normalization helpers
- [ ] Export via `return { extract_config = ..., coerce_value = ..., as_list = ... }`
- [ ] In `term.lua`, load with `local config_mod = dofile(FILTER_DIR .. "config.lua")`
- [ ] Replace direct function calls with `config_mod.extract_config(...)` etc.
- [ ] Update Lua tests to `dofile()` the module directly (no more regex extraction)
- [ ] Run all Lua tests
- [ ] Render a doc to verify config parsing still works

---

### 5.2 Extract `cell.lua`

**Why:** `parse_cell_options` (~119 lines) and `build_cell` (~152 lines) handle per-cell option parsing and request construction. They're self-contained and heavily tested. Extracting them simplifies both `term.lua` and the test setup.

**How:** Move to `_extensions/term/cell.lua`.

**TODO:**
- [ ] Create `_extensions/term/cell.lua`
- [ ] Move `parse_cell_options`, `build_cell`, and helpers they use (e.g., `escape_pattern`, dotted key expansion)
- [ ] Export via return table
- [ ] In `term.lua`, load with `local cell_mod = dofile(FILTER_DIR .. "cell.lua")`
- [ ] Update call sites in `term.lua`
- [ ] Update Lua tests to import from `cell.lua` directly
- [ ] Run all Lua tests
- [ ] Render a doc with various cell options to verify

---

### 5.3 Extract `cache.lua`

**Why:** Cache management (hash computation, file read/write, staleness checks) is I/O logic that's independent of the filter pipeline. Separating it clarifies what's pure logic vs. side effects and makes cache behavior testable.

**How:** Move to `_extensions/term/cache.lua`.

**TODO:**
- [ ] Create `_extensions/term/cache.lua`
- [ ] Move cache hash computation, cache file read, cache file write
- [ ] Move the `pandoc.pipe()` invocation that calls the Rust binary (this is the "engine run" step)
- [ ] Export: `M.run(request_json, config) -> results` (checks cache, runs binary if miss, writes cache)
- [ ] In `term.lua`, replace inline cache logic with `cache_mod.run(...)`
- [ ] Verify caching works: render twice, confirm second uses cache
- [ ] Verify `cache: false` bypasses correctly
- [ ] Run Lua tests

---

### 5.4 Extract `style.lua`

**Why:** Style resolution — merging document-level, format-level, and cell-level styles — is repeated logic that touches colorschemes, font-size, chrome, and line options. Centralizing it reduces the ~60 LOC of duplicated merge patterns.

**How:** Move to `_extensions/term/style.lua`.

**TODO:**
- [ ] Create `_extensions/term/style.lua`
- [ ] Move style extraction, merging, and format-override application logic
- [ ] Define `M.resolve(doc_style, cell_style, format_overrides) -> merged_style`
- [ ] Export via return table
- [ ] In `term.lua`, replace inline style merging with `style_mod.resolve(...)`
- [ ] Run Lua tests
- [ ] Render a doc with format-specific style overrides (e.g., `html: { colorscheme: nord }`) to verify

---

### 5.5 Slim down `term.lua`

**Why:** After extracting config, cell, cache, and style modules, `term.lua` should be a thin orchestrator: load modules, register the Pandoc filter, collect blocks, call engine, inject results and dependencies. Target: under 300 lines.

**How:** Verify that all logic has been properly delegated and `term.lua` is just glue.

**TODO:**
- [ ] Verify `term.lua` only contains: module loading, `FILTER_DIR` constant, Pandoc filter registration, block collection, engine invocation, result injection, CSS/JS dependency injection
- [ ] Remove any remaining inline logic that belongs in a module
- [ ] Verify line count is under 300
- [ ] Run full Quarto render of docs site
- [ ] Run all Lua tests
- [ ] Run E2E tests

---

## Phase 6: Duplication Reduction

Eliminate repeated code patterns in source and tests.

---

### 6.1 Consolidate Rust source duplication

**Why:** ~90-140 LOC of duplicated patterns in render/style logic, assertion building, and line-spec resolution across `session.rs`, `renderer.rs`, and `output_transform.rs`. The `RenderBackend` trait partially addresses rendering duplication, but assertion building and line-spec parsing remain repeated.

**How:** Identify the 3-4 most-repeated patterns and extract shared helpers.

**TODO:**
- [ ] Audit: grep for duplicated assertion-building patterns across test files and `session.rs`
- [ ] Extract a shared `fn build_assertion_result(pattern, output, cell_id) -> AssertionResult` if pattern repeats 3+ times
- [ ] Identify duplicated style/config default construction — extract `impl Default` or builder methods
- [ ] The `LineSelector` extraction (Phase 4.4) handles the line-spec duplication
- [ ] Run `cargo test` after each change

---

### 6.2 Consolidate Lua test helpers

**Why:** All 4 Lua test files repeat: pass/fail counters, `assert_eq`/`assert_true`/`assert_nil` helpers, `term.lua` path discovery, source loading, function extraction via string patterns, `mock_block`, and final report/exit logic. This makes new tests expensive to write.

**How:** Extract shared utilities into a single `helpers.lua` module. Now that functions are being extracted into real modules (Phase 5), tests can import directly instead of regex-extracting from source.

**TODO:**
- [ ] Enhance existing `tests/lua/helpers.lua` with:
  - [ ] `assert_eq(actual, expected, msg)` with diff output on failure
  - [ ] `assert_true(value, msg)`
  - [ ] `assert_nil(value, msg)`
  - [ ] `assert_not_nil(value, msg)`
  - [ ] `pass_count`, `fail_count` tracking
  - [ ] `finish(test_name)` — prints summary, exits with code
  - [ ] `load_module(name)` — dofile's a module from the extension directory
  - [ ] `mock_block(text, classes)` — creates a mock Pandoc CodeBlock
  - [ ] `term_meta(opts)` — builds a metadata table for `extract_config`
- [ ] Refactor `test_build_cell.lua` to use helpers + direct module import
- [ ] Refactor `test_coerce_value.lua` to use helpers
- [ ] Refactor `test_extract_config.lua` to use helpers
- [ ] Refactor `test_parse_cell_options.lua` to use helpers
- [ ] Run all Lua tests to confirm they still pass

---

### 6.3 Consolidate Rust test support module

**Why:** Multiple Rust source files repeat `avt::Vt::builder().size(...).feed_str(...)` for creating test terminal lines, `RenderedLine { html, text }` construction, and default style/theme fixtures. Each new test file re-invents these.

**How:** Expand the existing `src/test_support.rs` module (already exists, behind `#[cfg(test)]`) with shared builders.

**TODO:**
- [ ] Add to `src/test_support.rs`:
  - [ ] `pub fn vt_line(ansi: &str, cols: usize) -> avt::Line`
  - [ ] `pub fn rendered(text: &str) -> RenderedLine` — plain text, markup = escaped text
  - [ ] `pub fn rendered_markup(markup: &str, text: &str) -> RenderedLine`
  - [ ] `pub fn rendered_lines(texts: &[&str]) -> Vec<RenderedLine>`
  - [ ] `pub fn default_config() -> Config` — minimal valid config
  - [ ] `pub fn numbered_lines(n: usize) -> Vec<RenderedLine>`
- [ ] Refactor `src/output_transform.rs` tests to use helpers (already uses `make_rendered` — consolidate)
- [ ] Refactor `src/renderer.rs` tests to use helpers
- [ ] Refactor `src/latex.rs` tests to use helpers
- [ ] Refactor `src/typst.rs` tests to use helpers
- [ ] Run `cargo test` — all pass

---

### 6.4 Replace hand-built JSON in integration tests

**Why:** `tests/binary_protocol.rs` constructs JSON request strings by hand ~15 times. This is verbose, error-prone, and fragile when the protocol changes (e.g., renaming `html` to `markup`).

**How:** Use `serde_json::json!` macro and helper functions for common patterns.

**TODO:**
- [ ] Add helper functions at the top of `tests/binary_protocol.rs`:
  - [ ] `fn default_config() -> Value` — minimal valid config
  - [ ] `fn request(cells: Vec<Value>) -> Value` — wraps cells in default config
  - [ ] `fn request_with_config(config: Value, cells: Vec<Value>) -> Value`
  - [ ] `fn cell(id: u32, code: &str) -> Value` — minimal cell with defaults
  - [ ] `fn cell_with_opts(id: u32, code: &str, opts: Value) -> Value`
  - [ ] `fn run_request(req: &Value) -> Value` — pipes JSON through binary, parses output
- [ ] Convert each existing test to use helpers
- [ ] Run `cargo test` to confirm all pass
- [ ] Verify failure output is still clear (shows which cell and what went wrong)

---

## Phase 7: Architecture & Robustness

Structural improvements that reduce drift and improve reliability.

---

### 7.1 Add cache versioning

**Why:** Cache keys are based on request JSON but not versioned by binary/filter version. After upgrading the extension, stale cached output may be served — causing confusion when behavior changes don't take effect until the cache is manually cleared.

**How:** Include a version hash (or the extension version string) in the cache key computation.

**TODO:**
- [ ] In `term.lua` (or `cache.lua` after extraction), locate cache key/hash computation
- [ ] Include the extension version (from `_extension.yml` or the `VERSION` constant from Phase 1.2) in the hash input
- [ ] Optionally include the binary's own version (run `quarto-term --version` or hash the binary)
- [ ] Verify: upgrade version → old cache entries are missed → fresh execution
- [ ] Verify: same version → cache still hits correctly
- [ ] Run Lua tests
- [ ] Run E2E tests with cache enabled

---

### 7.2 Add `OutputFormat` enum in Rust

**Why:** Format strings `"html"`, `"latex"`, `"typst"`, `"markdown"` are matched as raw strings across `protocol.rs`, `session.rs`, and all renderer modules. A typo compiles fine but produces wrong output silently. An enum catches this at compile time and makes adding/removing formats explicit.

**How:** Define `OutputFormat` enum with `Deserialize`, replace all string matching.

**TODO:**
- [ ] Define in `src/protocol.rs`:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
  #[serde(rename_all = "lowercase")]
  pub enum OutputFormat { Html, Latex, Typst, Markdown }
  ```
- [ ] Change `Config.format` from `String` to `OutputFormat`
- [ ] Update all format matching in `session.rs` — replace `"html"` string literals with `OutputFormat::Html` etc.
- [ ] Update `build_cell_output` (after rename) dispatch
- [ ] Update renderer selection logic
- [ ] Ensure unknown format in JSON → deserialization error with clear message
- [ ] Update `term.lua` to validate format string before sending (already lowercase)
- [ ] Run `cargo test` — fix compilation errors from the switch
- [ ] Run E2E tests

---

### 7.3 Guard `install-binary.ts` with `import.meta.main`

**Why:** The Deno install script exports helper functions but runs `main()` unconditionally on import. This makes it impossible to import the helpers for testing without triggering the full install flow.

**How:** Wrap the `main()` call in `if (import.meta.main)`.

**TODO:**
- [ ] In `tests/deno/install_binary_test.ts` (or wherever the script lives), locate the top-level `main()` call
- [ ] Wrap it: `if (import.meta.main) { main(); }`
- [ ] Verify the script still works when run directly: `deno run install-binary.ts`
- [ ] Verify it can be imported without side effects: `import { someHelper } from "./install-binary.ts"`
- [ ] Run Deno tests

---

### 7.4 Library crate extraction

**Why:** The Rust binary is a monolithic CLI. Integration tests must spawn the binary and pipe JSON, which is slow and makes debugging harder. A library crate would allow tests to import protocol/session/rendering directly and test at finer granularity.

**How:** Split into `quarto-term` (binary, thin CLI) and `quarto-term-lib` (library crate with all logic). The binary just does stdin/stdout I/O and calls into the library.

**TODO:**
- [ ] Create `src/lib.rs` that re-exports all modules
- [ ] Move `main()` logic into a `pub fn run(input: &str) -> String` in the library
- [ ] Keep `src/main.rs` as a thin wrapper: read stdin → `run()` → print stdout
- [ ] Update `Cargo.toml` to produce both `[[bin]]` and `[lib]`
- [ ] Update `tests/binary_protocol.rs` to optionally call `run()` directly (faster)
- [ ] Verify the binary still works end-to-end
- [ ] Run `cargo test`

---

## Phase 8: Test Infrastructure

Improve test coverage and reduce friction for adding new tests.

---

### 8.1 Create shared Lua test fixtures

**Why:** Adding a new Lua test requires: setting up pass/fail counters, loading term.lua, extracting functions, mocking pandoc globals, and formatting results. This overhead discourages test creation.

**How:** With modules extracted (Phase 5), tests can import directly. The fixture layer provides mock pandoc globals and assertion helpers.

**TODO:**
- [ ] In `tests/lua/helpers.lua`, add mock pandoc globals:
  - [ ] `pandoc.system.os` (default: `"darwin"`)
  - [ ] `pandoc.system.arch` (default: `"aarch64"`)
  - [ ] `pandoc.pipe` (mock that returns configurable JSON)
  - [ ] `pandoc.utils.stringify` (simplified version for testing)
  - [ ] `quarto.log.warning` (captures warnings for assertion)
- [ ] Add `mock_metadata(overrides)` — returns a full metadata table with sane defaults
- [ ] Add `mock_code_block(code, attrs)` — returns a Pandoc CodeBlock AST node
- [ ] Verify all existing tests still pass with the enhanced helpers
- [ ] Write one new test using the fixtures to validate the workflow

---

### 8.2 Consolidate shell E2E test runners

**Why:** `tests/e2e/run_e2e.sh` duplicates the Pandoc invocation pattern, pass/fail reporting, and Docker variant logic for each test case. Adding a new test requires copying ~20 lines of boilerplate.

**How:** Extract shared functions and make test cases data-driven.

**TODO:**
- [ ] Define a `run_test()` function: takes input .qmd content, expected output pattern, assertion type (contains/not-contains/regex)
- [ ] Define a `run_docker_test()` wrapper: sets up Docker-specific environment
- [ ] Define `assert_contains()`, `assert_not_contains()`, `assert_exit_code()`
- [ ] Convert each existing test case to call shared functions
- [ ] Add summary at end: N passed, M failed, with appropriate exit code
- [ ] Run the E2E suite to confirm all still pass
- [ ] Add one new test case using the shared functions to validate the pattern

---

### 8.3 Add Rust integration test helpers

**Why:** `tests/binary_protocol.rs` is the only integration test file. As the protocol grows (diagnostics envelope, new formats), more integration tests will be needed. A shared harness reduces per-test overhead.

**How:** Create a `tests/common/mod.rs` with shared setup.

**TODO:**
- [ ] Create `tests/common/mod.rs` with:
  - [ ] `fn binary_path() -> PathBuf` — finds the compiled binary
  - [ ] `fn run_binary(input: &str) -> (String, String, ExitStatus)` — runs binary, returns stdout/stderr/status
  - [ ] `fn run_request(request: &Value) -> BatchResponse` — serialize, run, deserialize
  - [ ] `fn assert_cell_ok(response: &BatchResponse, index: usize) -> &str`
  - [ ] `fn assert_cell_error(response: &BatchResponse, index: usize) -> &str`
- [ ] Update `tests/binary_protocol.rs` to use common helpers
- [ ] Run `cargo test`

---

## Phase 9: Documentation & Developer Experience

Improve docs accuracy and reduce maintenance burden.

---

### 9.1 Add shell presets documentation

**Why:** Shell profiles exist in the implementation but are under-documented. Users manually set `shell`/`prompt`/`ps2` for each non-bash shell, which is error-prone. The `docs/shells/` pages teach manual setup but don't mention the preset shortcut.

**How:** Document the `profile` option prominently and show it as the recommended approach.

**TODO:**
- [ ] In `docs/shells/index.qmd`, add a "Quick Start with Profiles" section at the top
- [ ] Show: `profile: python` is equivalent to setting shell + prompt + ps2 manually
- [ ] List all available profiles with their effective settings
- [ ] In each shell-specific page (`docs/shells/python.qmd`, etc.), add a note: "You can use `profile: python` instead of manual configuration"
- [ ] In `docs/reference.qmd`, add `profile` to the options table with valid values
- [ ] Render and verify

---

### 9.2 Remove generated docs from git tracking

**Why:** `docs/*_files/libs/` directories contain ~3 MiB of duplicated Quarto build output per page (Bootstrap, reveal.js, clipboard.js). These are generated artifacts that bloat the repo, create noisy diffs, and slow down clones.

**How:** Add to `.gitignore` and remove from git tracking. Deploy docs via CI.

**TODO:**
- [ ] Add to `.gitignore`:
  ```
  docs/_site/
  docs/*_files/
  ```
- [ ] Run `git rm -r --cached docs/_site/ docs/*_files/` to remove from tracking
- [ ] Verify docs build still works: `cd docs && quarto render`
- [ ] Verify the docs site deploys correctly (CI/GitHub Pages)
- [ ] Update contributing guide if it references committed site output

---

### 9.3 CLI parser for `quarto-term check`

**Why:** `src/main.rs:116` manually parses `--record` with custom arg iteration and a confusing predicate that mixes `&&` and `||` without parentheses. Adding future flags (e.g., `--format`, `--verbose`, `--timeout`) will make this increasingly fragile and bug-prone.

**How:** Either add `clap` for proper argument parsing, or at minimum extract a clean `parse_check_args` function with clear predicates.

**TODO:**
- [ ] Add `clap` to `Cargo.toml` as an optional dependency (feature-gated if binary size is a concern)
- [ ] Define a `CheckArgs` struct: `file: PathBuf`, `record: Option<PathBuf>`, `format: Option<String>`
- [ ] Implement CLI parsing with `clap::Parser` derive macro
- [ ] Add `--help` support with usage text
- [ ] Replace the manual arg parsing in `run_check`
- [ ] Add test: `quarto-term check --help` exits 0
- [ ] Add test: `quarto-term check` with no file → helpful error
- [ ] Add test: `quarto-term check nonexistent.qmd` → "file not found" error
- [ ] Run `cargo test`

---

## Priority & Sequencing

| Phase | Effort | Risk | Dependency |
|-------|--------|------|-----------|
| 1. Critical Fixes | Low | Low | None — independent tasks |
| 2. Dead Code Removal | Low | Low | None — independent tasks |
| 3. API Consistency | Medium | Low | Phase 1.3 (naming) should happen first |
| 4. Rust Structure | High | Medium | Phase 3.2 (validation) informs enum design |
| 5. Lua Structure | Medium | Medium | Phase 2 cleanup first (less code to move) |
| 6. Duplication | Medium | Low | Phases 4+5 first (modules exist to consolidate into) |
| 7. Architecture | Medium | Medium | Phases 4+5 first (structure supports changes) |
| 8. Test Infrastructure | Medium | Low | Phase 5 first (modules importable for tests) |
| 9. Docs & DX | Low | Low | Phase 3 first (correct names to document) |

**Recommended order:** 1 → 2 → 3 → 5 → 4 → 6 → 7 → 8 → 9
