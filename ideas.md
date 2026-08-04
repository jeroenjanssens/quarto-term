# Future Ideas

## Interactivity (HTML output)

- **Progressive reveal**: show one command at a time with a "Next" button or keyboard shortcut. Readers step through the session like a slideshow.
- **Animated playback**: render a "Play" button that replays the terminal session character-by-character in the browser (timing data from `delay`/`typing`). Like an inline asciinema player but zero dependencies.
- **Collapsible output**: for cells that produce 50+ lines, auto-collapse with "Show full output" toggle.

## Visual / Presentation

- **Annotations/arrows**: point to specific parts of output with callout arrows or highlight boxes. Like `#! annotate: "this is the error"` that renders a tooltip/arrow.
- **Diff rendering**: a `diff` echo mode that shows before/after with red/green highlighting. For "edit this file" tutorials.
- **SVG export**: render terminal output as a standalone SVG. Perfect for slides, README badges, or social media cards.
- **Split pane**: show two sessions side by side (e.g., client and server, or "wrong" vs "right" approach).

## Recording / Export

- **GIF/WebP export**: render the terminal session as an animated image. No player needed, works everywhere including GitHub READMEs.
- **asciinema import**: given a `.cast` file, convert it into `{term}` cells. Turn existing recordings into reproducible docs.
- **Reveal.js fragments**: in presentations, each command auto-maps to a fragment so it appears on click/advance.

## Caching / Performance

- **Parallel sessions**: cells in different documents (or explicitly marked independent) run in parallel during `quarto render`.
- **Session continuity across documents**: define a session once, use it across multiple pages (e.g., "Getting Started Part 1" and "Part 2" share the same shell state).

## Authoring Quality-of-Life

- **Platform conditionals**: `#| when: linux` — skip cells on other platforms. For cross-platform tutorials.
- **Expected-failure mode**: `#| expect-error: true` — cell is expected to fail (for teaching error handling). Render error output styled differently.
- **Retry on flake**: `#| retries: 3` — for cells that depend on network/timing.

## AI / Smart Features

- **Auto-truncate detection**: intelligently detect repetitive output (like 1000 lines of compilation) and auto-truncate with a message, without manual `truncate` specs.
- **Command explanation tooltips**: hover over a command to see a plain-English explanation (generated at build time, stored in the HTML).
