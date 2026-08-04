# Docker Support Implementation Plan

## Goal

Enable quarto-term to execute commands inside a Docker container, so documents like *Data Science at the Command Line* can be rendered with their full toolbox environment. Each document gets its own container lifecycle: started at the beginning of the render, stopped/removed at the end.

## Design Decision

Use `docker run -it --rm` as the PTY child process. `portable-pty`'s `CommandBuilder` spawns `docker` directly (not the container shell). Docker's `-i -t` flags allocate a container-side TTY and connect stdin/stdout, so the existing prompt detection, ANSI capture, and input injection work unchanged. The container is automatically removed when the PTY closes (`--rm`).

This avoids `docker exec` and lifecycle management outside the PTY — the container lives exactly as long as the shell session.

## Configuration

All keys live under `extensions: term: docker:` in YAML front matter or `_quarto.yml`. Docker config is **project/document-level only** — no per-cell Docker switching (one container per document).

### Keys

| Key | Type | Default | Docker Flag | Description |
|-----|------|---------|-------------|-------------|
| `image` | string | **(required)** | positional | Container image |
| `pull` | string | `"missing"` | — | Pull policy: `always`, `missing`, `never` |
| `platform` | string | (none) | `--platform` | Platform override (e.g., `linux/amd64`) |
| `workdir` | string | (none) | `--workdir` | Working directory inside container |
| `user` | string | (none) | `--user` | Run as `user[:group]` |
| `network` | string | (none) | `--network` | Network mode (`bridge`, `host`, `none`, named) |
| `memory` | string | (none) | `--memory` | Memory limit (e.g., `512m`, `2g`) |
| `cpus` | string | (none) | `--cpus` | CPU limit (e.g., `1.5`) |
| `name` | string | (none) | `--name` | Container name (debugging aid) |
| `ports` | list of strings | `[]` | `-p` each | Port mappings (e.g., `["8080:8080"]`) |
| `volumes` | list of strings | `[]` | `-v` each | Volume mounts (`host:container[:opts]`); relative host paths resolve against document CWD |
| `env` | map | `{}` | `--env` each | Environment variables passed to container |
| `args` | list of strings | `[]` | inserted raw | Escape hatch for arbitrary `docker run` flags |

### Config Examples

**Minimal:**

```yaml
extensions:
  term:
    shell: bash
    shell-args: ["--norc", "--noprofile"]
    docker:
      image: "python:3.12"
```

**Full-featured (Data Science at the Command Line style):**

```yaml
extensions:
  term:
    shell: zsh
    shell-args: ["--no-rcs"]
    prompt: "$"
    timeout: 30.0
    docker:
      image: "datasciencetoolbox/dsatcl2e"
      platform: "linux/amd64"
      pull: missing
      workdir: /home/dst
      user: "1000:1000"
      network: none
      memory: "2g"
      cpus: "2.0"
      volumes:
        - "./data:/data"
        - "./images:/images"
        - "./output:/output:rw"
      env:
        BAT_THEME: "ansi"
        MANROFFOPT: "-c"
      args: ["--security-opt=no-new-privileges"]
```

**Project-level in `_quarto.yml`:**

```yaml
extensions:
  term:
    shell: bash
    shell-args: ["--norc", "--noprofile"]
    prompt: "$"
    timeout: 30.0
    docker:
      image: "rocker/r-ver:4.3"
      pull: missing
      workdir: /home/rstudio
      volumes:
        - "./data:/home/rstudio/data"
```

## Implementation

### 1. `src/protocol.rs` — Add DockerConfig struct

```rust
#[derive(Debug, Deserialize, Clone)]
pub struct DockerConfig {
    pub image: String,
    #[serde(default = "default_pull_policy")]
    pub pull: String,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub workdir: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub network: Option<String>,
    #[serde(default)]
    pub memory: Option<String>,
    #[serde(default)]
    pub cpus: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub ports: Vec<String>,
    #[serde(default)]
    pub volumes: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub args: Vec<String>,
}

fn default_pull_policy() -> String {
    "missing".to_string()
}
```

Add to `Config` struct:

```rust
#[serde(default)]
pub docker: Option<DockerConfig>,
```

### 2. `src/session.rs` — Docker command building

Three new free functions:

#### `check_docker_available()` → Result<(), TermError>

Run `docker info --format '{{.ServerVersion}}'` to verify Docker is installed and the daemon is running. Fail fast with clear error messages.

#### `maybe_pull_image(docker, verbose)` → Result<(), TermError>

Enforce pull policy:
- `"never"`: skip
- `"missing"`: run `docker image inspect <image>` — if exits 0, skip; otherwise fall through to pull
- `"always"`: pull unconditionally

Pull with `docker pull <image>`. If `platform` is set, add `--platform <platform>`. Report failures via `TermError::SpawnFailed`.

#### `build_docker_command(docker, config)` → CommandBuilder

Construct `docker run --rm -i -t [options...] <image> <shell> [shell-args...]`:

1. Base: `docker run --rm -i -t`
2. If `platform`: `--platform <platform>`
3. If `name`: `--name <name>`
4. If `workdir`: `--workdir <workdir>`
5. If `user`: `--user <user>`
6. If `network`: `--network <network>`
7. If `memory`: `--memory <memory>`
8. If `cpus`: `--cpus <cpus>`
9. For each port: `-p <port>`
10. For each volume: `-v <volume>`
11. For each docker.env entry: `--env KEY=VALUE`
12. For each config.env entry: `--env KEY=VALUE` (shell-level env, so the container shell sees them)
13. Always: `--env TERM=xterm-256color --env COLORTERM=truecolor --env LC_ALL=en_US.UTF-8`
14. For each arg in docker.args: insert raw
15. Positional: `<image>`
16. Positional: `<shell> [shell-args...]`

#### Modified `PtySession::new` branching

```rust
let cmd = if let Some(ref docker) = config.docker {
    check_docker_available()?;
    maybe_pull_image(docker, config.verbose)?;
    build_docker_command(docker, config)
} else {
    // existing local shell CommandBuilder logic (unchanged)
    ...
};
```

The rest of `PtySession::new` (PTY allocation, reader thread, VT init, wait_for_prompt, init commands) is completely unchanged.

### 3. `_extensions/term/term.lua` — Extract docker config

Add docker block parsing in `extract_config()` (after the `env` block, before the return):

- Read all keys from `term_meta["docker"]`
- Resolve relative volume host paths using `pandoc.system.get_working_directory()`
- Only set `config.docker` if `image` is present (ignore docker block without image)
- Ensure `docker.ports`, `docker.volumes`, `docker.args` use `pandoc.List({})` when empty (encode as JSON arrays)

### 4. `src/main.rs` — Verbose logging

When `config.verbose` and `config.docker` are both set, print the image name and volume count to stderr before session start.

### 5. Tests

#### Unit tests: `src/protocol.rs`

- `docker_config_minimal_deserialization` — image only, verify defaults
- `docker_config_full_deserialization` — all fields populated
- `docker_config_absent_is_none` — no docker key → `None`
- `docker_config_pull_default` — verify default is `"missing"`

#### Unit tests: `tests/lua/test_extract_config.lua`

- Docker config with image → extracted correctly
- Docker config without image → ignored (config.docker is nil)
- Volume relative path resolution → host path becomes absolute
- Docker env map → passed through

#### Integration test: `tests/binary_protocol.rs`

- `docker_config_propagates_to_error` — pass a non-existent image with `pull: "never"` → verify error is non-empty (not a panic). This test works whether Docker is installed or not (either "docker not found" or "image not found").

#### E2E test: `tests/e2e/run_e2e.sh`

Add a Docker section gated by `docker info >/dev/null 2>&1`:

- `echo_in_container` — basic `echo` in a `bash:5` container
- `state_persists` — export a variable in one cell, read it in the next
- `volume_mount` — write a file to a mounted volume, verify it exists on host after render

Skip with a message when Docker is unavailable.

### 6. Documentation

#### `docs/reference.qmd`

- Add `docker` row to "Document-Level Options" table: `docker | (none) | Run commands in a Docker container (see [Docker](docker.qmd))`
- Add `docker` to the "All Options" table with Project/Document columns checked

#### `docs/docker.qmd` (new page)

Sections:
1. **Overview** — one container per document, automatic lifecycle
2. **Configuration** — full key table with descriptions
3. **Examples** — minimal, full-featured, `_quarto.yml` project defaults
4. **Volume Mounts** — relative path resolution, read-only mounts
5. **Shell Configuration** — the image must have the configured shell; interaction with shell-args
6. **Pull Policies** — when each is appropriate
7. **Error Handling** — Docker not installed, daemon not running, image not found, shell not in image
8. **Platform Notes** — Apple Silicon + linux/amd64, Rosetta

#### `docs/_quarto.yml`

Add Docker page to navbar:

```yaml
- href: docker.qmd
  text: Docker
```

## File Change Summary

| File | Change |
|------|--------|
| `src/protocol.rs` | Add `DockerConfig` struct + field on `Config` + unit tests |
| `src/session.rs` | Add `check_docker_available`, `maybe_pull_image`, `build_docker_command` + branch in `new()` |
| `src/main.rs` | Verbose logging for docker mode |
| `_extensions/term/term.lua` | Docker config extraction in `extract_config()` |
| `tests/binary_protocol.rs` | Docker error propagation test |
| `tests/lua/test_extract_config.lua` | Docker extraction tests |
| `tests/e2e/run_e2e.sh` | Docker-gated E2E tests |
| `docs/reference.qmd` | Add docker to option tables |
| `docs/docker.qmd` | New dedicated page |
| `docs/_quarto.yml` | Add Docker page to navbar |

## TODO

- [x] protocol.rs: Add `DockerConfig` struct
- [x] protocol.rs: Add `docker: Option<DockerConfig>` to Config
- [x] protocol.rs: Add `default_pull_policy` function
- [x] protocol.rs: Add unit tests (4 tests)
- [x] session.rs: Add `check_docker_available()`
- [x] session.rs: Add `maybe_pull_image()`
- [x] session.rs: Add `build_docker_command()`
- [x] session.rs: Branch in `PtySession::new`
- [x] main.rs: Verbose logging for docker mode
- [x] term.lua: Extract docker config from metadata
- [x] term.lua: Resolve relative volume paths
- [x] term.lua: Ensure arrays encode as JSON arrays
- [x] tests/binary_protocol.rs: Docker error propagation test
- [x] tests/lua/test_extract_config.lua: Docker extraction tests
- [x] tests/e2e/run_e2e.sh: Docker-gated E2E tests
- [x] docs/docker.qmd: New documentation page
- [x] docs/reference.qmd: Add docker to option tables
- [x] docs/_quarto.yml: Add Docker page to navbar
- [x] Verify `cargo test` passes
- [x] Verify E2E tests pass (with and without Docker)
