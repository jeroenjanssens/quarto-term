# quarto-term

A [Quarto](https://quarto.org) extension that provides persistent, interactive terminal sessions across code cells. Commands run in a real shell with a PTY, so state (variables, working directory, background processes) carries over between cells exactly as it would in an interactive terminal.

## Why quarto-term?

Quarto's built-in `bash` engine executes each code cell in an isolated subshell. That means:

- Variables, `cd`, and other state don't persist between cells.
- You can't show interactive TUI applications (htop, vim, nyancat).
- Output is plain text with no ANSI color support.
- There's no way to send special keys (Ctrl-C, arrow keys, Enter separately from a command).
- You can't produce terminal recordings alongside your document.

quarto-term solves all of these by running a single persistent shell session that all cells share. It captures output using a real terminal emulator (including colors, cursor movement, and fullscreen apps), and can optionally produce [asciicast](https://docs.asciinema.org/) or [termshow](https://posit-dev.github.io/great-docs/user-guide/terminal-recordings.html) recordings for playback.

## Features

- **Persistent sessions** -- State carries across cells (variables, working directory, shell history).
- **ANSI color rendering** -- Faithfully renders colored output as HTML/LaTeX.
- **Fullscreen capture** -- Capture TUI apps like htop, vim, or nyancat.
- **Special keys** -- Send Ctrl-C, arrow keys, Enter, Escape, and more.
- **Human typing simulation** -- Configurable speed and error rate for realistic recordings.
- **Terminal recordings** -- Produce `.cast` (asciicast v2) or `.termshow` files alongside your document.
- **Line options** -- Control timing, key behavior, and prompt expectations per line.
- **Multi-line commands** -- Automatic PS2 continuation prompt detection.
- **Themes** -- Built-in color themes (solarized-dark, dracula, nord, tokyo-night, and more).
- **Multiple output formats** -- HTML, LaTeX/PDF, and Markdown.

## Installation

```bash
quarto add jeroenjanssens/quarto-term
```

This installs the extension with pre-built binaries for macOS (ARM and Intel), Linux (x86_64 and ARM), and Windows. No additional setup needed.

To build from source instead (requires [Rust](https://rustup.rs/)):

```bash
cargo build --release
```

## Quick Start

````markdown
---
title: "My Terminal Demo"
format: html
engine: markdown
term:
  shell: zsh
---

```{term}
echo 'Hello, world!'
```

```{term}
export NAME="Quarto"
```

```{term}
echo "Hello, $NAME"
```
````

The `engine: markdown` line tells Quarto to skip its built-in execution engines. The `term` key holds all configuration.

### Using with knitr or Jupyter

For documents that only contain terminal cells, `engine: markdown` is recommended. However, if you need to mix `{term}` cells with `{r}` or `{python}` cells, you can use knitr or Jupyter as the engine with some extra setup.

**Jupyter** works without changes. `{term}` blocks are not recognized as kernel code and pass through to the Lua filter.

**knitr** requires registering a pass-through engine so it doesn't try to execute `{term}` blocks. Add a setup chunk at the top of your document:

````markdown
```{r}
#| include: false
knitr::knit_engines$set(term = function(options) {
  knitr:::one_string(c("```{term}", options$code, "```"))
})
```
````

This tells knitr to emit `{term}` blocks unchanged, allowing the Lua filter to process them.

Note that `{term}` cells are always executed after all knitr/Jupyter cells have finished. The terminal session runs during the Pandoc filter phase, not during engine execution.

## Document-Level Options

Set these under `term:` in your YAML front matter:

| Option | Default | Description |
|--------|---------|-------------|
| `shell` | `zsh` | Shell to use |
| `shell-args` | auto | Shell arguments (defaults: `--no-rcs` for zsh, `--norc --noprofile` for bash) |
| `prompt` | `$` | Literal prompt string. Auto-sets `PS1` and builds the matching regex. |
| `prompt-regex` | (derived) | Raw regex override for prompt detection |
| `cols` | `80` | Terminal width |
| `rows` | `24` | Terminal height |
| `ansi` | `true` | Render ANSI colors in output |
| `timeout` | `10.0` | Seconds to wait for prompt before error |
| `spacing` | `false` | Add blank lines between commands in output |
| `trailing-spaces` | `false` | Preserve trailing whitespace in output |
| `marker` | `#!` | Prefix for line option annotations |
| `typing` | `false` | Human typing simulation (see below) |
| `record` | (none) | File path to record session (`.cast` or `.termshow`) |
| `theme` | (none) | Color theme name, or `{light: ..., dark: ...}` for auto-switching |
| `theme-bg` | (from theme) | Background color override (hex) |
| `theme-fg` | (from theme) | Foreground color override (hex) |
| `fontsize` | (none) | Font size (or per-format map: `{html: "0.85em", pdf: "0.75em"}`) |
| `verbose` | `false` | Print execution details to stderr |
| `init` | (none) | Shell script to source at session start |
| `env` | `{}` | Additional environment variables |

## Chunk Options

Set these with `#|` at the top of a cell:

| Option | Default | Description |
|--------|---------|-------------|
| `label` | (none) | Chunk label (shown in verbose mode) |
| `echo` | `terminal` | Output mode: `terminal`, `source`, `true`, or `false` |
| `output` | `true` | Show terminal output |
| `fullscreen` | `false` | Capture the entire terminal screen |
| `scroll` | `!fullscreen` | Include scrollback in capture |
| `keep-last-prompt` | `false` | Keep the trailing prompt in output |
| `ansi` | (from config) | Override ANSI rendering for this cell |
| `spacing` | (from config) | Override spacing for this cell |
| `trailing-spaces` | (from config) | Override trailing space handling |
| `typing` | (from config) | Override typing simulation |
| `timeout` | (from config) | Override timeout for this cell |
| `hold` | (none) | Seconds to wait after cell completes (captures ongoing output) |
| `highlight` | `bash` | Syntax highlighting language (for `echo: source`) |
| `marker` | (from config) | Override line marker for this cell |

## Line Options

Traditional code engines just *execute* code -- they send an entire block to an interpreter and capture whatever comes back. quarto-term is different: it *simulates a human typing at a terminal*. Each line is sent to the shell individually, keystroke by keystroke, just as you would type it yourself.

This distinction matters because a real terminal session has *timing*. You might type a command, wait for output to settle, then send a keystroke to interact with what's on screen. You can't express that in a flat code block -- but you can with line options.

Line options give you control over *how* each line is delivered to the shell: whether it's typed as literal characters or interpreted as a key name, whether Enter is pressed, how long to pause before or after, and whether to wait for the prompt to return. This is what makes it possible to drive interactive applications, interrupt long-running commands, and produce recordings with natural pacing.

Append options to any line using the marker (default `#!`):

````markdown
```{term}
sleep 100 #! expect-prompt: false
ctrl-c #! literal: false, delay: 0.5, expect-prompt: true
```
````

| Option | Default | Description |
|--------|---------|-------------|
| `literal` | `true` | Send text as typed characters. Set to `false` for key names. |
| `enter` | (from literal) | Press Enter after the line |
| `expect-prompt` | (from enter) | Wait for prompt after the line |
| `delay` | `0` | Seconds to wait *before* executing this line |
| `hold` | `0.1` | Seconds to wait after executing (captures output) |

## Special Keys

When `literal: false`, these key names are recognized:

- `enter` (also `return`, `cr`), `tab`, `space`
- `escape` (also `esc`), `backspace` (also `bs`), `delete` (also `del`)
- `up`, `down`, `left`, `right`, `home`, `end`
- `page-up` (also `pageup`), `page-down` (also `pagedown`), `insert`
- `ctrl-c`, `ctrl-d`, `ctrl-z`, `ctrl-l`, etc. (also `c-c` shorthand)
- `f1` through `f12`

Any other text is sent as raw bytes (e.g., `q` sends the letter q without pressing Enter).

## Human Typing Simulation

Simulate realistic human typing for recordings:

```yaml
term:
  typing:
    mode: human
    speed: 100        # characters per minute
    error-rate: 0.02  # probability of a typo per character
```

When a "typo" occurs, the simulated typist hits a QWERTY-adjacent key and then corrects with backspace. Timing follows a log-normal distribution with bigram-aware adjustments.

Disable typing for individual cells with `#| typing: false`.

## Fullscreen Applications

Capture TUI apps by splitting the interaction across cells:

````markdown
```{term}
#| fullscreen: true
#| hold: 3.0
htop
```

```{term}
#| echo: false
#| output: false
q #! literal: false
```
````

The first cell launches htop and captures the screen after 3 seconds. The second cell sends `q` to quit (hidden from output).

## Terminal Recordings

Record your session for playback:

```yaml
term:
  record: "session.cast"       # asciicast v2 format
  # or
  record: "session.termshow"   # termshow format (generates .termshow.yml)
```

Asciicast files can be played with [asciinema](https://asciinema.org/). Termshow files work with the [termshow](https://github.com/posit-dev/great-docs/) player.

## Themes

Built-in themes:

- `catppuccin-mocha`
- `dracula`
- `gruvbox-dark`
- `nord`
- `one-dark`
- `solarized-dark`
- `solarized-light`
- `tokyo-night`

Single theme:

```yaml
term:
  theme: solarized-dark
```

### Light/Dark Mode

For documents or websites that support Quarto's light/dark toggle, provide both themes:

```yaml
format:
  html:
    theme:
      light: flatly
      dark: darkly
term:
  theme:
    light: solarized-light
    dark: solarized-dark
```

The terminal blocks automatically switch themes when the user toggles the page between light and dark mode.

## Multi-Line Commands

Multi-line constructs (loops, heredocs, etc.) work automatically. quarto-term detects PS2 continuation prompts:

````markdown
```{term}
for i in 1 2 3; do
  echo "Item $i"
done
```
````

## Examples

See the [`examples/`](examples/) directory and the demo files (`demo.qmd`, `nyancat.qmd`) for complete working examples.

## License

MIT
