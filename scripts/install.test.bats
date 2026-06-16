#!/usr/bin/env bats
#
# install.test.bats — integration tests for scripts/install.sh.
#
# Strategy: generate fixture tarballs at setup time, serve them via a local
# Python HTTP server on a random port, and point install.sh at that server
# via KDASH_BASE_URL + KDASH_LATEST_URL. Each test owns a fresh install dir
# under a per-test temp directory.
#
# Run: bats scripts/install.test.bats
# Requires: bats-core, python3, tar, curl-or-wget, sha256sum-or-shasum.

INSTALL_SH_REL="scripts/install.sh"

# ---- fixture helpers --------------------------------------------------------

# Compute SHA-256 of a file using whichever tool is available.
_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    shasum -a 256 "$1" | awk '{print $1}'
  fi
}

# Detect the native kdash asset suffix from the host's uname output. Mirrors
# detect_asset_suffix() in install.sh for the common test hosts.
_native_suffix() {
  case "$(uname -s)" in
    Linux)
      if ldd --version 2>&1 | grep -qi musl; then libc=musl; else libc=gnu; fi
      case "$(uname -m)" in
        x86_64|amd64)  [ "$libc" = musl ] && echo "linux-musl" || echo "linux" ;;
        aarch64|arm64) [ "$libc" = musl ] && echo "aarch64-musl" || echo "aarch64-gnu" ;;
        *) echo "unsupported-host" ;;
      esac
      ;;
    Darwin)
      case "$(uname -m)" in
        x86_64) echo "macos" ;;
        arm64|aarch64) echo "macos-arm64" ;;
        *) echo "unsupported-host" ;;
      esac
      ;;
    *) echo "unsupported-host" ;;
  esac
}

# Build the fixture tree under $TEST_TMP/fixtures:
#   fixtures/releases/v0.2.0/kdash-${SUFFIX}.tar.gz   (flat: bare kdash binary)
#   fixtures/releases/v0.2.0/kdash-${SUFFIX}.sha256   (shasum -a 256 output)
#   fixtures/api/latest.json
# The fake binary in the tarball is a shell script that echoes "kdash 0.2.0".
_build_fixtures() {
  version_bare="0.2.0"
  tag="v${version_bare}"

  build_dir="$TEST_TMP/build"
  mkdir -p "$build_dir"
  cat >"$build_dir/kdash" <<EOF
#!/bin/sh
echo "kdash ${version_bare}"
EOF
  chmod +x "$build_dir/kdash"

  rel_dir="$TEST_TMP/fixtures/releases/${tag}"
  mkdir -p "$rel_dir"
  tar -czf "$rel_dir/kdash-${SUFFIX}.tar.gz" -C "$build_dir" kdash

  sha=$(_sha256 "$rel_dir/kdash-${SUFFIX}.tar.gz")
  printf '%s  %s\n' "$sha" "kdash-${SUFFIX}.tar.gz" > "$rel_dir/kdash-${SUFFIX}.sha256"

  mkdir -p "$TEST_TMP/fixtures/api"
  printf '{"tag_name": "%s", "name": "kdash %s"}\n' "$tag" "$version_bare" \
    > "$TEST_TMP/fixtures/api/latest.json"
}

# Start a Python HTTP server in $TEST_TMP/fixtures on a free port; export PORT,
# SERVER_PID, KDASH_BASE_URL, KDASH_LATEST_URL.
#
# Lifecycle note: the subshell uses `exec` so it becomes the python process,
# making $! the python PID directly. Without `exec`, killing $SERVER_PID would
# only kill the subshell and orphan the python child, leaking ports per test.
_start_server() {
  PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1]); s.close()')
  ( cd "$TEST_TMP/fixtures" && exec python3 -m http.server "$PORT" --bind 127.0.0.1 >/dev/null 2>&1 ) &
  SERVER_PID=$!

  # Poll for server readiness.
  ready=0
  i=0
  while [ "$i" -lt 50 ]; do
    if curl -sf "http://127.0.0.1:$PORT/" >/dev/null 2>&1; then
      ready=1
      break
    fi
    sleep 0.1
    i=$((i + 1))
  done
  if [ "$ready" -ne 1 ]; then
    echo "fixture server failed to start on port $PORT" >&2
    return 1
  fi

  export PORT SERVER_PID
  export KDASH_BASE_URL="http://127.0.0.1:$PORT/releases"
  export KDASH_LATEST_URL="http://127.0.0.1:$PORT/api/latest.json"
}

# ---- bats lifecycle ---------------------------------------------------------

setup() {
  SUFFIX=$(_native_suffix)
  if [ "$SUFFIX" = "unsupported-host" ]; then
    skip "unsupported test host: $(uname -s) $(uname -m)"
  fi
  export SUFFIX

  TEST_TMP=$(mktemp -d)
  export TEST_TMP
  export KDASH_INSTALL_DIR="$TEST_TMP/bin"
  export KDASH_QUIET=1

  _build_fixtures
  _start_server

  REPO_ROOT="$(cd "$BATS_TEST_DIRNAME/.." && pwd)"
  INSTALL_SH="$REPO_ROOT/$INSTALL_SH_REL"
  export INSTALL_SH
}

teardown() {
  if [ -n "${SERVER_PID:-}" ]; then
    kill -TERM "$SERVER_PID" 2>/dev/null || true
    for _ in 1 2 3; do
      if ! kill -0 "$SERVER_PID" 2>/dev/null; then break; fi
      sleep 0.1
    done
    kill -KILL "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
  rm -rf "${TEST_TMP:-}"
}

# ---- happy paths ------------------------------------------------------------

@test "happy path: install latest on native target" {
  run "$INSTALL_SH"
  [ "$status" -eq 0 ]
  [ -x "$KDASH_INSTALL_DIR/kdash" ]
  run "$KDASH_INSTALL_DIR/kdash" --version
  [ "$status" -eq 0 ]
  [ "$output" = "kdash 0.2.0" ]
}

@test "happy path: --version flag pins to a specific tag" {
  run "$INSTALL_SH" --version "v0.2.0"
  [ "$status" -eq 0 ]
  [ -x "$KDASH_INSTALL_DIR/kdash" ]
}

@test "happy path: KDASH_VERSION env pins to a specific tag" {
  KDASH_VERSION="v0.2.0" run "$INSTALL_SH"
  [ "$status" -eq 0 ]
  [ -x "$KDASH_INSTALL_DIR/kdash" ]
}

@test "happy path: bare version (no leading v) is accepted" {
  run "$INSTALL_SH" --version "0.2.0"
  [ "$status" -eq 0 ]
  [ -x "$KDASH_INSTALL_DIR/kdash" ]
}

@test "happy path: --prefix overrides install directory" {
  alt_dir="$TEST_TMP/alt-bin"
  # --prefix must override the env-var default.
  KDASH_INSTALL_DIR= run "$INSTALL_SH" --prefix "$alt_dir"
  [ "$status" -eq 0 ]
  [ -x "$alt_dir/kdash" ]
}

@test "happy path: BIN_DIR legacy env is honored as the install dir" {
  alt_dir="$TEST_TMP/legacy-bin"
  KDASH_INSTALL_DIR= BIN_DIR="$alt_dir" run "$INSTALL_SH"
  [ "$status" -eq 0 ]
  [ -x "$alt_dir/kdash" ]
}

# ---- idempotence ------------------------------------------------------------

@test "idempotence: re-running the same version is a no-op" {
  run "$INSTALL_SH"
  [ "$status" -eq 0 ]

  first_inode=$(ls -i "$KDASH_INSTALL_DIR/kdash" | awk '{print $1}')

  run "$INSTALL_SH"
  [ "$status" -eq 0 ]

  second_inode=$(ls -i "$KDASH_INSTALL_DIR/kdash" | awk '{print $1}')

  # Same inode = file wasn't replaced. The script's idempotence branch
  # short-circuits before touching the destination.
  [ "$first_inode" = "$second_inode" ]
}

# ---- PATH hint --------------------------------------------------------------

@test "PATH hint: warning shown when install dir is not on PATH" {
  PATH="/usr/bin:/bin" KDASH_QUIET= run "$INSTALL_SH"
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "not on your"
}

@test "PATH hint: warning omitted when install dir is on PATH" {
  PATH="$KDASH_INSTALL_DIR:/usr/bin:/bin" KDASH_QUIET= run "$INSTALL_SH"
  [ "$status" -eq 0 ]
  ! echo "$output" | grep -q "not on your"
}

# ---- error paths ------------------------------------------------------------

@test "checksum mismatch: refuses install with exit 2" {
  # Replace the sidecar digest with a valid-but-wrong 64-hex hash.
  rel_dir="$TEST_TMP/fixtures/releases/v0.2.0"
  printf '%s  %s\n' "$(printf '0%.0s' $(seq 1 64))" "kdash-${SUFFIX}.tar.gz" \
    > "$rel_dir/kdash-${SUFFIX}.sha256"

  run "$INSTALL_SH"
  [ "$status" -eq 2 ]
  [ ! -e "$KDASH_INSTALL_DIR/kdash" ]
}

@test "malformed checksum file: non-hex digest exits 2" {
  rel_dir="$TEST_TMP/fixtures/releases/v0.2.0"
  printf 'not-a-valid-hash  kdash-%s.tar.gz\n' "$SUFFIX" \
    > "$rel_dir/kdash-${SUFFIX}.sha256"

  run "$INSTALL_SH"
  [ "$status" -eq 2 ]
}

@test "missing tarball: sidecar present but asset absent exits 1" {
  rel_dir="$TEST_TMP/fixtures/releases/v0.2.0"
  rm -f "$rel_dir/kdash-${SUFFIX}.tar.gz"

  run "$INSTALL_SH"
  [ "$status" -eq 1 ]
  [ ! -e "$KDASH_INSTALL_DIR/kdash" ]
}

@test "nonexistent version exits 1" {
  run "$INSTALL_SH" --version "v9.9.9"
  [ "$status" -eq 1 ]
  [ ! -e "$KDASH_INSTALL_DIR/kdash" ]
}

@test "unknown flag exits 64" {
  run "$INSTALL_SH" --unknown-flag
  [ "$status" -eq 64 ]
}

@test "missing --version argument exits 64" {
  run "$INSTALL_SH" --version
  [ "$status" -eq 64 ]
}

@test "missing --prefix argument exits 64" {
  run "$INSTALL_SH" --prefix
  [ "$status" -eq 64 ]
}

@test "--help exits 0 and prints usage" {
  KDASH_QUIET= run "$INSTALL_SH" --help
  [ "$status" -eq 0 ]
  echo "$output" | grep -q "install.sh — install kdash"
}

# ---- platform refusal / resolution via uname + ldd stubs --------------------

# Create a stub uname earlier on PATH so install.sh sees the spoofed OS/arch.
# The stub falls through to the real uname for any flag it doesn't care about,
# so other tools that happen to call uname during the test still work.
_install_uname_stub() {
  spoofed_s="$1"
  spoofed_m="$2"
  stub_dir="$TEST_TMP/stubs"
  mkdir -p "$stub_dir"
  real_uname=$(command -v uname)
  cat >"$stub_dir/uname" <<EOF
#!/bin/sh
case "\$1" in
  -s) echo "${spoofed_s}" ;;
  -m) echo "${spoofed_m}" ;;
  *) ${real_uname} "\$@" ;;
esac
EOF
  chmod +x "$stub_dir/uname"
  echo "$stub_dir"
}

_install_ldd_stub() {
  ldd_output="$1"
  stub_dir="$TEST_TMP/stubs"
  mkdir -p "$stub_dir"
  cat >"$stub_dir/ldd" <<EOF
#!/bin/sh
printf '%s\n' '${ldd_output}'
EOF
  chmod +x "$stub_dir/ldd"
  echo "$stub_dir"
}

@test "Windows host is refused with exit 64 and a clear message" {
  stub_dir=$(_install_uname_stub "MINGW64_NT-10.0" "x86_64")
  PATH="$stub_dir:$PATH" KDASH_QUIET= run "$INSTALL_SH"
  [ "$status" -eq 64 ]
  echo "$output" | grep -q "Windows"
}

@test "unsupported Linux arch is refused with exit 64" {
  stub_dir=$(_install_uname_stub "Linux" "riscv64")
  PATH="$stub_dir:$PATH" KDASH_QUIET= run "$INSTALL_SH"
  [ "$status" -eq 64 ]
  echo "$output" | grep -q "unsupported Linux arch"
}

@test "Linux musl x86_64 resolves to the linux-musl asset" {
  uname_stub=$(_install_uname_stub "Linux" "x86_64")
  ldd_stub=$(_install_ldd_stub "musl libc (x86_64)")
  PATH="$ldd_stub:$uname_stub:$PATH" KDASH_QUIET= run "$INSTALL_SH" --version "v9.9.9"
  [ "$status" -eq 1 ]
  echo "$output" | grep -q "kdash-linux-musl"
}

@test "Linux armv7 glibc resolves to the armv7-gnu asset" {
  uname_stub=$(_install_uname_stub "Linux" "armv7l")
  ldd_stub=$(_install_ldd_stub "ldd (GNU libc) 2.39")
  PATH="$ldd_stub:$uname_stub:$PATH" KDASH_QUIET= run "$INSTALL_SH" --version "v9.9.9"
  [ "$status" -eq 1 ]
  echo "$output" | grep -q "kdash-armv7-gnu"
}

@test "Linux armv6 musl resolves to the armv6-musl asset" {
  uname_stub=$(_install_uname_stub "Linux" "armv6l")
  ldd_stub=$(_install_ldd_stub "musl libc (armhf)")
  PATH="$ldd_stub:$uname_stub:$PATH" KDASH_QUIET= run "$INSTALL_SH" --version "v9.9.9"
  [ "$status" -eq 1 ]
  echo "$output" | grep -q "kdash-armv6-musl"
}

@test "unsupported macOS arch is refused with exit 64" {
  stub_dir=$(_install_uname_stub "Darwin" "powerpc")
  PATH="$stub_dir:$PATH" KDASH_QUIET= run "$INSTALL_SH"
  [ "$status" -eq 64 ]
  echo "$output" | grep -q "unsupported macOS arch"
}

@test "exotic OS is refused with exit 64" {
  stub_dir=$(_install_uname_stub "Plan9" "x86_64")
  PATH="$stub_dir:$PATH" KDASH_QUIET= run "$INSTALL_SH"
  [ "$status" -eq 64 ]
  echo "$output" | grep -q "unsupported OS"
}
