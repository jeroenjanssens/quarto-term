# Docker mode end-to-end tests. Sourced by run_e2e.sh when
# QUARTO_TERM_TEST_DOCKER=1; uses its FILTER, PASS, and FAIL.
#
# Expected strings never appear literally in the cell source (they are
# computed by the command), so a test can't pass just because the source
# is echoed back.

IMAGE="bash:5"
WORK_DIR=$(mktemp -d)
trap 'rm -rf "$WORK_DIR"' EXIT

# docker_doc TERM_YAML DOCKER_YAML CELLS
# Builds a document; TERM_YAML and DOCKER_YAML are extra lines (indented
# to their level) for the `term:` and `docker:` blocks. Defaults are only
# added for keys that TERM_YAML doesn't set, to avoid duplicate YAML keys.
docker_doc() {
  local term="$1" defaults="" line key
  while IFS= read -r line; do
    key="${line%%:*}"
    case "$term" in
      *"$key:"*) ;;
      *) defaults="$defaults$line"$'\n' ;;
    esac
  done <<'DEFAULTS'
    shell: bash
    shell-args: ["--norc", "--noprofile"]
    timeout: 30.0
    prompt: "$"
DEFAULTS
  printf -- '---
extensions:
  term:
    cache: false
%s%s
    docker:
%s
---

%s
' "$defaults" "$1" "$2" "$3"
}

# render DOC: renders to HTML, with stderr (where cell errors go) included
render() {
  printf '%s' "$1" | timeout 120 pandoc -f markdown -t html --lua-filter "$FILTER" 2>&1 || true
}

pass() {
  echo "  PASS: $1"
  PASS=$((PASS + 1))
}

fail() {
  echo "  FAIL: $1"
  [ -n "${2:-}" ] && echo "    $2"
  FAIL=$((FAIL + 1))
}

# check NAME OUTPUT EXPECTED...: passes if OUTPUT contains every EXPECTED
check() {
  local name="$1" output="$2"
  shift 2
  if printf '%s' "$output" | grep -qF "Duplicate key"; then
    fail "$name" "test document has duplicate YAML keys"
    return
  fi
  for expected in "$@"; do
    if ! printf '%s' "$output" | grep -qF -- "$expected"; then
      fail "$name" "expected '$expected', got: $(printf '%s' "$output" | head -c 400)"
      return
    fi
  done
  pass "$name"
}

# check_absent NAME OUTPUT UNEXPECTED
check_absent() {
  if printf '%s' "$2" | grep -qF -- "$3"; then
    fail "$1" "should not contain '$3'"
  else
    pass "$1"
  fi
}

cell() {
  printf '``` {.term}\n%s\n```\n\n' "$1"
}

# --- Basics ---

out=$(render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never" "$(cell 'echo "docker-$((6 * 7))"')$(cell 'grep "^ID=" /etc/os-release')")")
check "runs commands inside the container" "$out" "docker-42" "ID=alpine"

out=$(render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never" "$(cell 'export DOCKER_VAR="container$((40 + 2))"')$(cell 'echo "$DOCKER_VAR"')")")
check "state persists across cells" "$out" "container42"

docker_doc "" "      image: \"$IMAGE\"
      pull: never" "$(cell 'touch /tmp/qt-marker')" > "$WORK_DIR/first.md"
render "$(cat "$WORK_DIR/first.md")" > /dev/null
out=$(render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never" "$(cell '[ -e /tmp/qt-marker ]; echo "marker-status-$?"')")")
check "each document gets a fresh container" "$out" "marker-status-1"

# --- Image default shell and entrypoint ---

# doc_without_shell DOCKER_YAML CELLS: like docker_doc, but without `shell`
# and `shell-args`, so the image's own ENTRYPOINT/CMD starts the shell
doc_without_shell() {
  printf -- '---
extensions:
  term:
    cache: false
    timeout: 30.0
    docker:
%s
---

%s
' "$1" "$2"
}

out=$(render "$(doc_without_shell "      image: \"$IMAGE\"
      pull: never" "$(cell 'echo "default-shell-$((5 * 5)) $(basename "$0")"')")")
check "no shell: image's default command (bash) is used" "$out" "default-shell-25 bash"
check_absent "no shell: no cell errors" "$out" "error:"

# An image whose ENTRYPOINT is the shell itself
SHELL_EP_IMAGE="quarto-term-test-shell-entrypoint:latest"
printf 'FROM %s\nENTRYPOINT ["/usr/local/bin/bash"]\n' "$IMAGE" \
  | docker build -q -t "$SHELL_EP_IMAGE" - > /dev/null

out=$(render "$(doc_without_shell "      image: \"$SHELL_EP_IMAGE\"
      pull: never" "$(cell 'echo "ep-default-$((2 + 2))"')")")
check "shell entrypoint, no shell: works" "$out" "ep-default-4"

out=$(render "$(docker_doc '    timeout: 5.0' "      image: \"$SHELL_EP_IMAGE\"
      pull: never" "$(cell 'echo hi')")")
check "shell entrypoint + explicit shell: fails with hint" "$out" \
  "session failed to start" "ENTRYPOINT" "docker.entrypoint"

out=$(render "$(docker_doc "" "      image: \"$SHELL_EP_IMAGE\"
      pull: never
      entrypoint:" "$(cell 'echo "ep-cleared-$((3 + 3))"')")")
check "shell entrypoint + explicit shell + empty entrypoint: works" "$out" "ep-cleared-6"

docker image rm -f "$SHELL_EP_IMAGE" > /dev/null 2>&1 || true

# --- Cache key includes the image ---

CACHE_IMAGE="quarto-term-test-cache:latest"
CACHE_DIR="$WORK_DIR/cache-test"
mkdir -p "$CACHE_DIR"
cache_doc() {
  printf -- '---
extensions:
  term:
    cache: true
    timeout: 30.0
    shell: sh
    shell-args: []
    docker:
      image: "%s"
      pull: never
---

``` {.term}
echo "run-$(od -An -N4 -tu4 /dev/urandom | tr -d " ")"
```
' "$CACHE_IMAGE"
}
render_in_cache_dir() {
  (cd "$CACHE_DIR" && render "$(cache_doc)") | grep -o 'run-[0-9][0-9]*' | tail -1
}

docker tag "$IMAGE" "$CACHE_IMAGE"
first=$(render_in_cache_dir)
second=$(render_in_cache_dir)
if [ -n "$first" ] && [ "$first" = "$second" ]; then
  pass "cache: same image is a cache hit"
else
  fail "cache: same image is a cache hit" "first='$first' second='$second'"
fi

docker pull -q alpine:3 > /dev/null 2>&1 || true
docker tag alpine:3 "$CACHE_IMAGE"
third=$(render_in_cache_dir)
if [ -n "$third" ] && [ "$third" != "$second" ]; then
  pass "cache: new image behind the same tag is a cache miss"
else
  fail "cache: new image behind the same tag is a cache miss" "second='$second' third='$third'"
fi
docker image rm -f "$CACHE_IMAGE" > /dev/null 2>&1 || true

# --- Interaction with term options ---

out=$(render "$(docker_doc '    prompt: "%"' "      image: \"$IMAGE\"
      pull: never" "$(cell 'echo "prompt-$((1 + 1))"')")")
check "custom prompt" "$out" "prompt-2" 'data-prompt="% "'

out=$(render "$(docker_doc '    style:
      cols: 100
      rows: 30' "      image: \"$IMAGE\"
      pull: never" "$(cell 'stty size')")")
check "terminal size from style.cols/rows" "$out" ">30 100<"

out=$(render "$(docker_doc '    env:
      QT_TERM_ENV: "from-term-env"' "      image: \"$IMAGE\"
      pull: never" "$(cell 'echo "term-env=$QT_TERM_ENV"')")")
check "term env is set in the container" "$out" "term-env=from-term-env"

out=$(render "$(docker_doc '    shell: sh
    shell-args: []' "      image: \"$IMAGE\"
      pull: never" "$(cell 'echo "sh-$((2 * 21)) shell=$0"')")")
check "shell: sh" "$out" "sh-42 shell=sh"

out=$(render "$(docker_doc '    shell: sh
    shell-args: []' "      image: \"alpine:3\"" "$(cell 'echo "alpine-$((3 * 3))"')")")
check "image without an entrypoint script (pull: missing)" "$out" "alpine-9"

# --- Docker options ---

out=$(render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never
      workdir: /srv/qt-workdir" "$(cell 'pwd')")")
check "workdir" "$out" "/srv/qt-workdir"

out=$(render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never
      user: \"1234:5678\"" "$(cell 'echo "uid=$(id -u) gid=$(id -g)"')")")
check "user" "$out" "uid=1234 gid=5678"

out=$(render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never" "$(cell '[ -e /sys/class/net/eth0 ]; echo "eth0-status-$?"')")")
check "network: default has eth0" "$out" "eth0-status-0"

out=$(render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never
      network: none" "$(cell '[ -e /sys/class/net/eth0 ]; echo "eth0-status-$?"')")")
check "network: none" "$out" "eth0-status-1"

out=$(render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never
      memory: 256m
      cpus: 1.5" "$(cell 'echo "mem=$(cat /sys/fs/cgroup/memory.max) cpu=$(cat /sys/fs/cgroup/cpu.max)"')")")
check "memory and cpus" "$out" "mem=268435456" "cpu=150000 100000"

out=$(render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never
      env:
        QT_DOCKER_ENV: \"from-docker-env\"
        QT_SECOND: \"second value\"" "$(cell 'echo "docker-env=$QT_DOCKER_ENV, $QT_SECOND"')")")
check "docker env" "$out" "docker-env=from-docker-env, second value"

out=$(render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never
      args: [\"--hostname=qt-args-host\", \"--add-host\", \"qt-alias:10.1.2.3\"]" "$(cell 'echo "host=$(hostname)"; grep qt-alias /etc/hosts | cut -f1')")")
check "args (single and two-part flags)" "$out" "host=qt-args-host" "10.1.2.3"

out=$(render "$(docker_doc "" "      image: \"$IMAGE\"
      platform: \"linux/amd64\"" "$(cell 'echo "arch=$(uname -m)"')")")
check "platform" "$out" "arch=x86_64"

# name and ports are only visible from the host while the container runs
container="quarto-term-e2e-$$"
render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never
      name: \"$container\"
      ports: [\"80\"]" "$(cell 'sleep 4')")" > /dev/null &
render_pid=$!
port_mapping=""
for _ in $(seq 1 40); do
  port_mapping=$(docker port "$container" 80 2>/dev/null || true)
  [ -n "$port_mapping" ] && break
  sleep 0.25
done
wait "$render_pid" || true
if [ -n "$port_mapping" ]; then
  pass "name and ports"
else
  fail "name and ports" "no container named $container with port 80 published"
fi
sleep 1
if docker ps -a --format '{{.Names}}' | grep -qx "$container"; then
  fail "container is removed after rendering" "$container still exists"
else
  pass "container is removed after rendering"
fi

# --- Volumes ---

mkdir -p "$WORK_DIR/abs" "$WORK_DIR/ro" "$WORK_DIR/doc/rel"
echo "abs-content-$$" > "$WORK_DIR/abs/host.txt"
echo "ro-content-$$" > "$WORK_DIR/ro/host.txt"
echo "rel-content-$$" > "$WORK_DIR/doc/rel/host.txt"

out=$(render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never
      volumes:
        - \"$WORK_DIR/abs:/abs\"
        - \"$WORK_DIR/ro:/ro:ro\"" "$(cell 'cat /abs/host.txt /ro/host.txt')$(cell 'echo from-container > /abs/container.txt && printf "wr%s\\n" ite-ok')$(cell 'touch /ro/nope 2>&1 | grep -o "Read-only"')")")
check "volumes: read multiple mounts" "$out" "abs-content-$$" "ro-content-$$"
check "volumes: write" "$out" "write-ok"
check "volumes: :ro is read-only" "$out" "Read-only"
if [ "$(cat "$WORK_DIR/abs/container.txt" 2>/dev/null)" = "from-container" ]; then
  pass "volumes: written file visible on host"
else
  fail "volumes: written file visible on host"
fi
if [ -e "$WORK_DIR/ro/nope" ]; then
  fail "volumes: :ro leaves host untouched"
else
  pass "volumes: :ro leaves host untouched"
fi

mkdir -p "$WORK_DIR/shared"
echo "parent-content-$$" > "$WORK_DIR/shared/host.txt"
out=$(cd "$WORK_DIR/doc" && render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never
      volumes:
        - \"./rel:/rel\"
        - \"../shared:/shared\"" "$(cell 'cat /rel/host.txt /shared/host.txt')")")
check "volumes: ./ and ../ resolve against working directory" "$out" "rel-content-$$" "parent-content-$$"

# Named volumes are managed by Docker and outlive the container
named_volume="quarto-term-e2e-$$"
docker volume rm "$named_volume" >/dev/null 2>&1 || true
out=$(cd "$WORK_DIR/doc" && render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never
      volumes:
        - \"$named_volume:/named\"" "$(cell 'echo "named-$((7 * 6))" > /named/f.txt')")")
out=$(cd "$WORK_DIR/doc" && render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never
      volumes:
        - \"$named_volume:/named\"" "$(cell 'cat /named/f.txt')")")
check "volumes: named volume persists across documents" "$out" "named-42"
if [ -e "$WORK_DIR/doc/$named_volume" ]; then
  fail "volumes: named volume is not a host directory" "$WORK_DIR/doc/$named_volume was created"
else
  pass "volumes: named volume is not a host directory"
fi
docker volume rm "$named_volume" >/dev/null 2>&1 || true

# --- Pull policies ---

out=$(render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: always" "$(cell 'echo "always-$((5 + 5))"')")")
check "pull: always" "$out" "always-10"

out=$(render "$(docker_doc "" "      image: \"quarto-term-e2e-missing-$$:1\"
      pull: never" "$(cell 'echo hi')")")
check "pull: never with missing image fails" "$out" "cell 1 error: session failed to start"

out=$(render "$(docker_doc "" "      image: \"quarto-term-e2e-missing-$$:1\"" "$(cell 'echo hi')")")
check "pull: missing with unknown image reports pull failure" "$out" "failed to pull image 'quarto-term-e2e-missing-$$:1'"

# --- Combination (the full-featured example from the docs) ---

mkdir -p "$WORK_DIR/full/data"
echo "full-data-$$" > "$WORK_DIR/full/data/input.txt"
out=$(cd "$WORK_DIR/full" && render "$(docker_doc '    env:
      QT_TERM_ENV: "combo-term"
    style:
      cols: 90
      rows: 20' "      image: \"$IMAGE\"
      pull: missing
      workdir: /home/qt
      user: \"1000:1000\"
      network: none
      memory: 512m
      cpus: \"2.0\"
      volumes:
        - \"./data:/data:ro\"
      env:
        QT_DOCKER_ENV: \"combo-docker\"
      args: [\"--security-opt=no-new-privileges\", \"--hostname=qt-combo\"]" "$(cell 'echo "pwd=$(pwd) uid=$(id -u) host=$(hostname) envs=$QT_TERM_ENV,$QT_DOCKER_ENV"')$(cell 'echo "mem=$(cat /sys/fs/cgroup/memory.max) cpu=$(cat /sys/fs/cgroup/cpu.max) size=$(stty size)"')$(cell '[ -e /sys/class/net/eth0 ]; echo "eth0-status-$?"; cat /data/input.txt')")")
check "combination of options" "$out" \
  "pwd=/home/qt uid=1000 host=qt-combo envs=combo-term,combo-docker" \
  "mem=536870912 cpu=200000 100000 size=20 90" \
  "eth0-status-1" \
  "full-data-$$"
check_absent "combination: no cell errors" "$out" "error:"

# --- Errors ---

out=$(render "$(docker_doc '    shell: zsh
    shell-args: []
    timeout: 5.0' "      image: \"$IMAGE\"
      pull: never" "$(cell 'echo hi')")")
check "shell not in image fails" "$out" "cell 1 error: session failed to start"

out=$(printf '%s' "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never" "$(cell 'echo hi')")" \
  | timeout 60 env PATH="$WORK_DIR/no-docker" "$(command -v pandoc)" -f markdown -t html --lua-filter "$FILTER" 2>&1 || true)
check "docker not installed" "$out" "docker not found in PATH"

out=$(DOCKER_HOST="unix://$WORK_DIR/no-daemon.sock" render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never" "$(cell 'echo hi')")")
check "docker daemon not reachable" "$out" "docker daemon is not running or not accessible"

# Some Docker versions exit 0 from `docker info` with an empty server
# version when the daemon is unreachable; simulate that with a fake docker
mkdir -p "$WORK_DIR/fake-docker"
printf '#!/bin/sh\nexit 0\n' > "$WORK_DIR/fake-docker/docker"
chmod +x "$WORK_DIR/fake-docker/docker"
out=$(PATH="$WORK_DIR/fake-docker:$PATH" render "$(docker_doc "" "      image: \"$IMAGE\"
      pull: never" "$(cell 'echo hi')")")
check "docker daemon not reachable (docker info exits 0)" "$out" "docker daemon is not running or not accessible"
