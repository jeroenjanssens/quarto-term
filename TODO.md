# TODO

Open tasks, roughly in priority order.

1. **Windows is untested for Docker mode.**
   - Volume path resolution is covered by unit tests with a Windows working directory, but whether Docker Desktop accepts the resulting paths has not been checked on a real machine.
   - Whether the PTY plus `docker run -i -t` approach works on Windows at all is unverified. CI only runs on `ubuntu-latest`.
2. **Make the cache key robust.**
   - Using the binary's *file size* in the key means a rebuild of the same size reuses stale results; this has bitten local development.
   - Options: hash the binary contents (cost: reading ~3 MB per render; could memoize by mtime+size), or use mtime+size.
   - The key code is at the top of `M.run` in `_extensions/term/cache.lua`.
3. **Strengthen the non-Docker e2e tests.**
   - Many tests in `tests/e2e/run_e2e.sh` (e.g. "echo hello produces output" expecting `hello`) look for strings that also appear in the cell *source*. They pass as long as anything renders.
   - Apply the same rule as `docker_tests.sh`: make each command compute its expected value.
   - Also consider making the base suite fail on pandoc duplicate-key warnings.
4. **Untested and undocumented: `docker.env` vs `extensions.term.env` precedence.**
   - In `build_docker_command`, `docker.env` is passed first and `config.env` later (and again via `env`), so term env wins on conflicts.
   - Decide whether that's the intended behavior, then document and test it.
5. **The stale local binary:** `_extensions/term/bin/quarto-term-aarch64-apple-darwin` (Aug 4) on my machine. It's gitignored and harmless now that the tests set `QUARTO_TERM_BINARY`, but it will still be picked up by ad hoc `pandoc --lua-filter` runs. Delete it once nothing depends on it.
6. **`docs/use-dev-binary.sh` hardcodes `aarch64-apple-darwin`** as the destination name. On other platforms (including CI's Linux runner) the copy is effectively unused, and the filter falls back to `../target/release`. It works, but it could derive the platform name or just set `QUARTO_TERM_BINARY`.
7. **Optional:** a navbar logo (`website.navbar.logo: logo.svg` in `docs/_quarto.yml`). It was skipped on purpose, but it may still be nice to have.
