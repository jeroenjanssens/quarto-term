# quarto-term


**quarto-term** is a [Quarto](https://quarto.org) extension that runs
commands in a real, persistent shell session. State carries across
cells, colors render faithfully, and you can drive fullscreen TUI apps,
all from your `.qmd` file.

quarto-term is great for:

- **Documentation and tutorials**: CLI tools, shell concepts,
  installation guides, git workflows, and API exploration with real
  output and persistent state
- **Technical books and courses**: faithful terminal rendering for any
  “type this, see that” sequence, including TUI walkthroughs, database
  demos, and data pipeline chains
- **DevOps and security**: runbooks, CTF writeups, and environment setup
  guides where colored tool output and multi-step sessions matter

```console
$ date
Sat Aug 15 14:30:22 CEST 2026

$ EXTENSION_NAME="quarto-term"
```

```console
$ cowsay "Hello, I'm rendered using $EXTENSION_NAME"
 _______________________________________
< Hello, I'm rendered using quarto-term >
 ---------------------------------------
        \   ^__^
         \  (oo)\_______
            (__)\       )\/\
                ||----w |
                ||     ||

$ htop
```

```text

    0[||||||||59.5%]   4[         0.0%]   7[          0.0%] 11[||        6.6%]
    1[||||||||51.3%]   5[         0.0%]   8[          0.0%] 12[||        1.3%]
    2[||||||| 48.3%]   6[         0.0%]   9[||       10.0%] 13[||        1.3%]
    3[||||||  42.0%]                     10[||        9.2%]
  Mem[|||||||||||||||||||||20.0G/24.0G] Tasks: 632, 4264 thr, 0 kthr; 1 runnin
  Swp[|||||||||||||||||||||2.47G/3.00G] Load average: 2.48 4.14 4.56
                                        Uptime: 7 days, 02:59:54

  [Main]
  PID USER       PRI  NI  VIRT   RES S  CPU%▽MEM%   TIME+  Command
 2052 jeroen      24   0 1577G  206M S   0.7  0.8 10:41.96 /Applications/Obsidia
 3616 jeroen      26   0  420G  419M S   0.6  1.7  2:25.16 claude --dangerously-
31502 jeroen      17   0  415G 53808 S   0.4  0.2  0:25.71 /Applications/Adobe I
22161 jeroen      17   0  464G 53072 S   0.3  0.2  0:23.19 /Applications/Positro
22165 jeroen      17   0 1860G  230M S   0.2  0.9  0:14.52 /Applications/Positro
31388 jeroen      24   0  426G  206M S   0.2  0.8  0:34.12 /Applications/Adobe I
22164 jeroen      17   0 1860G  245M S   0.1  1.0  0:13.15 /Applications/Positro
90050 jeroen      24   0  461G  215M S   0.1  0.9  1:47.73 /Applications/Utiliti
31469 jeroen      17   0  415G 40240 S   0.1  0.2  0:13.01 /Applications/Adobe I
31507 jeroen      17   0  455G 55136 S   0.1  0.2  0:05.97 /Applications/Adobe I
88757 jeroen      17   0  416G  168M S   0.1  0.7  0:11.82 /Applications/WhatsAp
33383 jeroen      17   0  415G 12304 S   0.1  0.0  0:04.80 /System/Library/Frame
F1Help  F2Setup F3SearchF4FilterF5Tree  F6SortByF7Nice -F8Nice +F9Kill  F10Quit
```

All of the above was produced from a single `.qmd` file using
quarto-term.

## Why quarto-term?

Quarto’s built-in `bash` engine executes each code cell in an isolated
subshell. That means:

- Variables, `cd`, and other state don’t persist between cells.
- You can’t show interactive TUI applications (htop, vim, nyancat).
- Output is plain text with no ANSI color support.
- There’s no way to send special keys (Ctrl-C, arrow keys).
- You can’t produce terminal recordings alongside your document.

quarto-term solves all of these by running a single persistent shell
session that all cells share. It captures output using a real terminal
emulator (including colors, cursor movement, and fullscreen apps), and
can optionally produce [asciicast](https://docs.asciinema.org/) or
[termshow](https://posit-dev.github.io/great-docs/user-guide/terminal-recordings.html)
recordings for playback.

## Features

- **Persistent sessions.** State carries across cells (variables,
  working directory, shell history).
- **ANSI color rendering.** Faithfully renders colored output as
  HTML/LaTeX.
- **Fullscreen capture.** Capture TUI apps like htop, vim, or nyancat.
- **Special keys.** Send Ctrl-C, arrow keys, Enter, Escape, and more.
- **Human typing simulation.** Configurable speed and error rate for
  realistic recordings.
- **Terminal recordings.** Produce `.cast` (asciicast v2) or `.termshow`
  files alongside your document.
- **Line options.** Control timing, key behavior, and prompt
  expectations per line.
- **Multi-line commands.** Automatic PS2 continuation prompt detection.
- **Styling.** Built-in colorschemes with light/dark mode support,
  fonts, and sizing.
- **Multiple output formats.** HTML, LaTeX/PDF, and Markdown.

## Quick install

``` bash
quarto add jeroenjanssens/quarto-term
```

On first render, the correct binary for your platform is automatically
downloaded from GitHub Releases and cached locally. Supported platforms:
macOS (ARM and Intel), Linux (x86_64 and ARM), and Windows.

See [Get
Started](https://jeroenjanssens.github.io/quarto-term/get-started.html)
for a complete walkthrough.
