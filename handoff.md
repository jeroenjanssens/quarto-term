# Handoff: quarto-term (October 30–31, 2026)

This document picks up from a working session on quarto-term: adding the logo, a local preview target, and a round of fixes and tests for Docker mode. It covers what changed, how the affected parts work now, the gotchas found along the way, and the tasks still open.

## Project at a glance

- **What it is:** a Quarto extension that runs `{term}` cells in a real, persistent shell session and renders the output, including colors and fullscreen TUIs.
- **Two halves:**
  - A Lua filter in `_extensions/term/`. `term.lua` is the entry point; `config.lua`, `cache.lua`, `engine.lua`, `cell.lua` and the others are loaded from it.
  - A Rust binary (`src/`) that drives the PTY sessions.
  - They talk over JSON: the filter pipes a request to the binary's stdin and reads a JSON array of results from its stdout. **Anything else written to the binary's stdout corrupts the response**; see "Gotchas".
- **Version:** 0.5.0, in both `_extensions/term/_extension.yml` and `Cargo.toml`. The latest tag is `v0.5.0`, and everything below came after it, so it's unreleased.
- **Website:** https://quarto-term.jeroenjanssens.com, built from `docs/` by `.github/workflows/publish-docs.yml` on pushes to `main` that touch `docs/**`.
- **Commits:** the repo owner commits straight to `main`. The user's instructions say commits must have Jeroen Janssens as the only author, with no co-author lines.

## What was done (all pushed to `main`, CI green)

| Commit | Summary |
|---|---|
| `b460042` | Logo added to the README, homepage, favicon, Apple touch icon, and Open Graph/Twitter card; `just preview` target; stale Get Started URL fixed in `docs/index.qmd` |
| `9f9b947` | The README is only regenerated on full renders (not during `quarto preview`) |
| `db82e05` | Docker mode fixed and tested end to end; failed results no longer cached; `QUARTO_TERM_BINARY` override |
| `4b91ebc` | Unreachable Docker daemon detected even when `docker info` exits 0 |
| `95e4e47` | Docker volume paths resolved like `docker run -v` (**breaking change**, see open task 1) |

### Logo and website (`b460042`)

- **Files** in `docs/`:
  - `logo.svg`: the original logo.
  - `logo.png`: 241×278, used as the social card image.
  - `favicon.png`: 96×96, the hex padded to a square.
  - `apple-touch-icon.png`: 180×180.
  - The PNGs were generated with `rsvg-convert` and `magick` from `logo.svg`.
- **`docs/_quarto.yml`:**
  - `site-url`, `favicon`, `image: logo.png`, `open-graph`, `twitter-card` (`card-style: summary`, because the logo is small and portrait).
  - An `include-in-header` link for the touch icon.
  - `apple-touch-icon.png` and `logo.png` listed under `project.resources`, because Quarto doesn't copy them on its own.
- **Homepage (`docs/index.qmd`):**
  - The logo lives *in the title* (`title: "quarto-term ![](logo.svg){.float-end ...}"`), so it lines up with the H1.
  - `pagetitle: "quarto-term"` keeps the browser tab and `og:title` clean.
  - `image: logo.png` is set on the page because otherwise Quarto picks the SVG as the share image, and social sites don't accept SVG.
- **README:** generated from `docs/index.qmd` by `docs/render-readme.sh`, a post-render step.
  - It renders GFM with `-M title:quarto-term`, so the website's image markup stays out of the README.
  - Then `sed` turns line 1 into `# quarto-term <img src="docs/logo.svg" align="right" height="139" .../>`, the pkgdown-style header.
  - The navbar logo was deliberately left out, because the text in the hex is unreadable at navbar size.
- **`just preview`:** runs `cargo build --release && quarto preview docs`. The release build matters because `docs/use-dev-binary.sh` copies `target/release/quarto-term` into the docs build.

### README regeneration (`9f9b947`)

`render-readme.sh` exits early unless `QUARTO_PROJECT_RENDER_ALL=1`.
- **Tested:** Quarto sets it only for full project renders (`quarto render docs`, which is what CI runs). `quarto preview` never sets it, neither at startup nor on re-renders.
- **Trade-off:** `quarto render docs/index.qmd` no longer updates the README.
- **Why:** before, every preview re-render rewrote `README.md` with fresh `date`/`htop` output.

### Docker mode (`db82e05`, `4b91ebc`, `95e4e47`)

Before this work, the Docker feature had never actually been tested end to end. The tests existed, but they were opt-in, CI never enabled them, and they failed when run. Fixed:

1. **Prompt never matched in containers** (`src/docker.rs`, `build_docker_command`).
   - **Cause:** quarto-term sets `PS1` and friends via `--env`, but images whose entrypoint is a bash script (like `bash:5`'s `docker-entrypoint.sh`) drop `PS1`/`PS2`, because non-interactive bash unsets them before `exec "$@"`. The shell showed `bash-5.3# ` and every session timed out.
   - **Fix:** the container command is now `<image> env K=V ... <shell> <args>`, with `config.env` sorted for deterministic argv. `--env` is kept too, so entrypoints still see the variables.
2. **`docker pull` output corrupted the JSON response.**
   - **Cause:** pull progress went to the binary's stdout ahead of the JSON. Every render that pulled (`pull: always`, or `pull: missing` with an absent image, e.g. the first CI render) silently produced unrendered cells.
   - **Fix:** `pull.stdout(std::io::stderr())`.
3. **Silent failure on invalid engine output** (`_extensions/term/cache.lua`). The new `decode()` helper prints `quarto-term: engine returned invalid output: ...` and returns nil. Invalid output is never cached.
4. **Failed results were cached forever** (`cache.lua`). Results containing any cell `error` are no longer written to the cache, so failing cells are retried on the next render.
5. **Daemon check** (`check_docker_available`). On GitHub's Ubuntu runners, `docker info --format '{{.ServerVersion}}'` exits 0 with empty output when the daemon is unreachable. The check now requires a non-empty version as well.
6. **Volume resolution** (`_extensions/term/config.lua`, new `M.resolve_volume(spec, cwd)`):

   | Host part | Example | Result |
   |---|---|---|
   | Starts with `.` | `./data`, `../shared`, `.`, `.hidden` | Resolved against the working directory |
   | Absolute path | `/tmp/cache` | Unchanged |
   | Windows drive path | `C:\data`, `C:/data` | Unchanged (previously split at the drive letter) |
   | Bare name | `myvol` | Unchanged, so Docker treats it as a named volume |

   Before this, any host part not starting with `/` got the working directory prepended, so named volumes were impossible. Each rule was checked against the Docker CLI's own behavior.

7. **Docs:**
   - `docs/docker.qmd`: Volume Mounts section rewritten. The error table was also corrected: a missing image with `pull: never`, or a shell missing from the image, fails right away with `session failed to start: shell process exited unexpectedly`, not with a timeout.
   - `docs/debugging.qmd`: now documents `QUARTO_TERM_BINARY` and the fact that failed results aren't cached.

### Test infrastructure (`db82e05`)

- **`QUARTO_TERM_BINARY`:** a new env var, checked first by `_extensions/term/engine.lua`'s `M.find()`, that overrides which binary the filter runs.
- **`tests/e2e/run_e2e.sh`:**
  - Exports `QUARTO_TERM_BINARY=target/debug/quarto-term` and uses `cache: false`. Before, e2e silently used whatever binary `engine.lua` found first (see "Gotchas") and could return cached results.
  - When `QUARTO_TERM_TEST_DOCKER=1`, it sources `tests/e2e/docker_tests.sh` (35 Docker tests). Once Docker tests are requested, a failed `docker pull bash:5` counts as a failure, not a skip.
- **`tests/e2e/docker_tests.sh`:**
  - **Coverage:** every Docker option, volumes (absolute, relative, `../`, `:ro`, several mounts, named volume across documents), all pull policies, quarto-term settings inside Docker (prompt, `style.cols`/`rows`, env, `sh`, an image without an entrypoint script), the docs' full-featured example as a combination test, and error cases (no docker in PATH, unreachable daemon via `DOCKER_HOST`, a fake `docker` whose `info` exits 0, missing image, missing shell).
  - **Design rules:**
    - Expected strings are *computed* by the command (e.g. `echo "docker-$((6 * 7))"` → `docker-42`), so a test can't pass just because the cell source is echoed into the HTML.
    - `docker_doc` adds the default `shell`/`shell-args`/`timeout`/`prompt` lines only when the test doesn't set them, because duplicate YAML keys let the default win silently.
    - `check` fails if pandoc warns about duplicate keys.
    - `name`/`ports` are checked from the host with `docker port` while a background render runs.
  - **Cleanup:** the script uses a `mktemp -d` work dir (removed on exit) and removes its named volume `quarto-term-e2e-$$`.
- **New tests elsewhere:**
  - 5 Rust unit tests for `build_docker_command` (`src/docker.rs`, `mod tests`).
  - `tests/lua/test_cache.lua`, which uses a fake engine script that counts calls.
  - 14 cases for `resolve_volume` in `tests/lua/test_extract_config.lua`.
- **CI:** `.github/workflows/test.yml` sets `QUARTO_TERM_TEST_DOCKER: "1"` on the e2e step.
- **Justfile:** new `test-e2e-docker` target (`just test` doesn't include Docker).
- **Mutation checks:** reverting each fix (env wrapper, pull stdout, daemon check, volume resolution) makes the corresponding tests fail.

**Current test counts:** 195 Rust unit tests, 30 Rust integration tests, Lua (build_cell 65, cache 9, coerce_value 12, extract_config 94, parse_cell_options 41), 10 Deno tests, and 48 e2e tests (13 base + 35 Docker). All pass locally and in CI.

## How to run things

```bash
just test              # Rust, Lua, Deno, e2e (no Docker)
just test-e2e-docker   # e2e including Docker; needs a running daemon, ~40s
just preview           # build release binary, then quarto preview docs
quarto render docs     # full site render; also regenerates README.md
```

## Gotchas learned the hard way

- **Binary lookup order** (`engine.lua`):
  1. `QUARTO_TERM_BINARY`
  2. `_extensions/term/bin/quarto-term-<platform>` (gitignored, downloaded by `install-binary.ts`)
  3. `target/release`
  4. `target/debug`

  On the owner's machine, `_extensions/term/bin/` holds a **stale binary from Aug 4** that speaks an older protocol (it returns `html` instead of `markup`). Anything that doesn't set `QUARTO_TERM_BINARY` silently uses it, and `target/release` beats `target/debug`.
- **The cache key** is `sha1(version, binary file size, request JSON)`, stored in `./.quarto-term-cache/` relative to the *working directory*. A rebuilt binary of the same size reuses stale cache entries. See open task 3.
- **The binary's stdout is the protocol channel.** Any subprocess the binary spawns must not inherit its stdout.
- **Quarto metadata changes** (even `-M title:...`) change the cache key, so terminal cells re-run and `README.md` gets new `date`/`htop` output. When regenerating the README only for a heading change, restore the body from `git show HEAD:README.md` to avoid churn.
- **Terminal size options** live under `style:` (`style.cols`, `style.rows`), not at the top level of `extensions.term`.
- **A running `quarto preview`** re-renders on every docs edit. That's harmless now for the README, but anything else a post-render script writes still happens.
- **Bash `PATH=... cmd`** uses the new PATH to look up `cmd` itself. To hide `docker` from the binary in a test, use `timeout 60 env PATH=... /abs/pandoc ...`.

## Open tasks

Roughly in priority order.

1. **Announce the breaking volume change in the next release.**
   - Bare names (`data:/data`) now mount a Docker named volume called `data`. Before, they mounted the `./data` directory. Docker creates the named volume silently, so affected users see an empty mount with no error. The fix for them is `./data:/data`.
   - Releases come from `.github/workflows/build-binaries.yml` with `gh release create --generate-notes`. Since commits go straight to `main` without PRs, the generated notes won't list this, so add it by hand (e.g. `gh release edit vX.Y.Z --notes ...`). Probably worth a minor version bump (`just release-minor`) rather than a patch.
   - The release would also ship the other Docker fixes, which matter to anyone who tried Docker mode with a bash-entrypoint image or a pull.
2. **Windows is untested for Docker mode.**
   - `resolve_volume` handles drive paths in the host part. But relative paths are joined as `cwd .. "/" .. host`, which gives mixed separators on Windows (`C:\Users\doc/data`). Docker Desktop probably accepts this, but nobody has verified it.
   - Whether the PTY plus `docker run -i -t` approach works on Windows at all is also unverified. CI only runs on `ubuntu-latest`.
3. **Make the cache key robust.**
   - Using the binary's *file size* in the key means a rebuild of the same size reuses stale results; this has bitten local development.
   - Options: hash the binary contents (cost: reading ~3 MB per render; could memoize by mtime+size), or use mtime+size.
   - The key code is at the top of `M.run` in `_extensions/term/cache.lua`.
4. **Strengthen the non-Docker e2e tests.**
   - Many tests in `tests/e2e/run_e2e.sh` (e.g. "echo hello produces output" expecting `hello`) look for strings that also appear in the cell *source*. They pass as long as anything renders.
   - Apply the same rule as `docker_tests.sh`: make each command compute its expected value.
   - Also consider making the base suite fail on pandoc duplicate-key warnings.
5. **Untested and undocumented: `docker.env` vs `extensions.term.env` precedence.**
   - In `build_docker_command`, `docker.env` is passed first and `config.env` later (and again via `env`), so term env wins on conflicts.
   - Decide whether that's the intended behavior, then document and test it.
6. **The stale local binary:** `_extensions/term/bin/quarto-term-aarch64-apple-darwin` (Aug 4) on the owner's machine. It's gitignored and harmless now that the tests set `QUARTO_TERM_BINARY`, but it will still be picked up by ad hoc `pandoc --lua-filter` runs. Deleting it is the owner's call.
7. **`docs/use-dev-binary.sh` hardcodes `aarch64-apple-darwin`** as the destination name. On other platforms (including CI's Linux runner) the copy is effectively unused, and the filter falls back to `../target/release`. It works, but it could derive the platform name or just set `QUARTO_TERM_BINARY`.
8. **Optional:** a navbar logo (`website.navbar.logo: logo.svg` in `docs/_quarto.yml`). It was skipped on purpose, but the owner may want it.
