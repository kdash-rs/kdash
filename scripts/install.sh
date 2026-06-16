#!/bin/sh
#
# install.sh — install kdash from GitHub Releases.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/kdash-rs/kdash/main/scripts/install.sh | sh
#   curl -fsSL https://github.com/kdash-rs/kdash/releases/latest/download/install.sh | sh
#
# Flags:
#   --version <vX.Y.Z>   Install a specific tag instead of the latest release.
#   --prefix <dir>       Install into <dir> instead of $HOME/.local/bin.
#   --quiet              Suppress progress chatter; errors still print.
#   -h, --help           Print this help and exit.
#
# Environment variables (equivalents to the flags above):
#   KDASH_VERSION       Same as --version.
#   KDASH_INSTALL_DIR   Same as --prefix.
#   BIN_DIR             Legacy alias for the install dir (getLatest.sh compat).
#   KDASH_QUIET=1       Same as --quiet.
#
# Test-only overrides (do not set unless running the bats suite):
#   KDASH_BASE_URL      Override the GH Releases download base URL.
#   KDASH_LATEST_URL    Override the GH API latest-release endpoint.
#
# Exit codes:
#   0   success
#   1   generic failure (download error, network, unknown)
#   2   checksum verification failed
#   64  unsupported platform or invalid usage
#
# Platform (uname -s, uname -m, libc) -> kdash release asset suffix:
#   linux x86_64  + glibc -> linux
#   linux x86_64  + musl  -> linux-musl
#   linux aarch64 + glibc -> aarch64-gnu
#   linux aarch64 + musl  -> aarch64-musl
#   linux armv6l  + glibc -> armv6-gnu
#   linux armv6l  + musl  -> armv6-musl
#   linux armv7l  + glibc -> armv7-gnu
#   linux armv7l  + musl  -> armv7-musl
#   darwin x86_64         -> macos
#   darwin arm64          -> macos-arm64
#
# Windows is handled by scripts/install.ps1 (irm | iex), not this script.
#
# This script must run under POSIX sh and Bash 3.2+ (macOS default). It avoids
# Bash 4-only features (mapfile, readarray, ${var,,}) so it works everywhere.

set -eu

REPO_DEFAULT="kdash-rs/kdash"
BIN_NAME="kdash"

# Test-only overrides; in production these resolve to GitHub's real endpoints.
BASE_URL="${KDASH_BASE_URL:-https://github.com/${REPO_DEFAULT}/releases/download}"
LATEST_URL="${KDASH_LATEST_URL:-https://api.github.com/repos/${REPO_DEFAULT}/releases/latest}"

# State populated from args / env later. BIN_DIR is honored for backward
# compatibility with the old deployment/getLatest.sh install path.
REQUESTED_VERSION="${KDASH_VERSION:-}"
INSTALL_DIR="${KDASH_INSTALL_DIR:-${BIN_DIR:-$HOME/.local/bin}}"
QUIET="${KDASH_QUIET:-}"

log() {
  if [ -z "$QUIET" ]; then
    printf '%s\n' "$*"
  fi
}

err() {
  printf 'error: %s\n' "$*" >&2
}

usage() {
  # Print the script header (lines starting with '#' up to the first blank line
  # after the shebang) so --help stays in sync with the docs above.
  sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'
}

# ----- argument parsing --------------------------------------------------------

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      if [ "$#" -lt 2 ]; then
        err "--version requires a tag argument (e.g., v2.0.0)"
        exit 64
      fi
      REQUESTED_VERSION="$2"
      shift 2
      ;;
    --version=*)
      REQUESTED_VERSION="${1#--version=}"
      shift
      ;;
    --prefix)
      if [ "$#" -lt 2 ]; then
        err "--prefix requires a directory argument"
        exit 64
      fi
      INSTALL_DIR="$2"
      shift 2
      ;;
    --prefix=*)
      INSTALL_DIR="${1#--prefix=}"
      shift
      ;;
    --quiet|-q)
      QUIET=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      err "unknown argument: $1"
      err "run with --help for usage"
      exit 64
      ;;
  esac
done

# ----- platform detection ------------------------------------------------------

# Emit the kdash release-asset suffix (the part after `kdash-` in the tarball
# name) for the host platform, or exit 64 if unsupported.
detect_asset_suffix() {
  uname_s=$(uname -s 2>/dev/null || echo unknown)
  uname_m=$(uname -m 2>/dev/null || echo unknown)

  case "$uname_s" in
    Linux)
      libc_flavor=$(detect_linux_libc)
      case "$uname_m" in
        x86_64|amd64)
          if [ "$libc_flavor" = "musl" ]; then echo "linux-musl"; else echo "linux"; fi
          ;;
        aarch64|arm64)
          if [ "$libc_flavor" = "musl" ]; then echo "aarch64-musl"; else echo "aarch64-gnu"; fi
          ;;
        armv6l|armv6)
          if [ "$libc_flavor" = "musl" ]; then echo "armv6-musl"; else echo "armv6-gnu"; fi
          ;;
        armv7l|armv7)
          if [ "$libc_flavor" = "musl" ]; then echo "armv7-musl"; else echo "armv7-gnu"; fi
          ;;
        *)
          err "unsupported Linux arch: $uname_m (supported: x86_64, aarch64, armv6l, armv7l)"
          exit 64
          ;;
      esac
      ;;
    Darwin)
      case "$uname_m" in
        x86_64) echo "macos" ;;
        arm64|aarch64) echo "macos-arm64" ;;
        *)
          err "unsupported macOS arch: $uname_m (supported: x86_64, arm64)"
          exit 64
          ;;
      esac
      ;;
    MINGW*|MSYS*|CYGWIN*|Windows_NT)
      err "Windows is not supported by install.sh — use install.ps1 instead:"
      err "  irm https://raw.githubusercontent.com/${REPO_DEFAULT}/main/scripts/install.ps1 | iex"
      exit 64
      ;;
    *)
      err "unsupported OS: $uname_s"
      exit 64
      ;;
  esac
}

detect_linux_libc() {
  if [ -f /etc/alpine-release ]; then
    echo "musl"
    return
  fi

  if command -v ldd >/dev/null 2>&1; then
    ldd_out=$(ldd --version 2>&1 || true)
    case "$ldd_out" in
      *musl*)
        echo "musl"
        return
        ;;
      *glibc*|*GLIBC*|*"GNU libc"*)
        echo "gnu"
        return
        ;;
    esac
  fi

  # Default to glibc when detection is inconclusive: Debian/Ubuntu/Fedora
  # land here when `ldd --version` emits a distro-specific preamble before
  # mentioning GLIBC, and the released gnu artefacts are the common case.
  echo "gnu"
}

# ----- network helpers ---------------------------------------------------------

# Wrap downloads so the rest of the script doesn't care whether curl or wget is
# present. Both are common on Linux + macOS; Alpine/Wolfi sometimes ship only
# wget. We require -fsSL semantics (fail-on-error, silent, follow redirects).
download() {
  url="$1"
  dest="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --retry 3 --retry-delay 1 -o "$dest" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -q -O "$dest" "$url"
  else
    err "neither curl nor wget is available on PATH; cannot download"
    exit 1
  fi
}

# ----- version resolution ------------------------------------------------------

resolve_version() {
  if [ -n "$REQUESTED_VERSION" ]; then
    # Normalise "v2.0.0" / "2.0.0" both work.
    case "$REQUESTED_VERSION" in
      v*) echo "$REQUESTED_VERSION" ;;
      *)  echo "v${REQUESTED_VERSION}" ;;
    esac
    return
  fi

  tmp=$(mktemp)
  if ! download "$LATEST_URL" "$tmp" 2>/dev/null; then
    rm -f "$tmp"
    err "could not fetch latest release info from $LATEST_URL"
    exit 1
  fi

  # Prefer jq when available; fall back to grep+sed. The grep fallback is
  # deliberately tolerant: it matches the first "tag_name" key with a string
  # value, which is the GitHub API contract.
  if command -v jq >/dev/null 2>&1; then
    tag=$(jq -r '.tag_name // empty' < "$tmp")
  else
    tag=$(grep -o '"tag_name"[[:space:]]*:[[:space:]]*"[^"]*"' "$tmp" \
          | head -1 \
          | sed -e 's/.*"tag_name"[[:space:]]*:[[:space:]]*"//' -e 's/"$//')
  fi
  rm -f "$tmp"

  if [ -z "$tag" ]; then
    err "could not parse tag_name from $LATEST_URL response"
    exit 1
  fi
  echo "$tag"
}

# ----- checksum verification --------------------------------------------------

# Compute SHA-256 of one file. macOS ships `shasum -a 256`; most Linux distros
# ship `sha256sum` (and often `shasum` too). Print the bare hex digest.
sha256_compute() {
  path="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$path" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$path" | awk '{print $1}'
  else
    err "neither sha256sum nor shasum is available; cannot verify download"
    exit 1
  fi
}

# ----- main ------------------------------------------------------------------

asset_suffix=$(detect_asset_suffix)
version=$(resolve_version)

tarball="${BIN_NAME}-${asset_suffix}.tar.gz"
tarball_url="${BASE_URL}/${version}/${tarball}"
# Each kdash asset ships a sibling <name>.sha256 sidecar (shasum -a 256 output).
sums_url="${BASE_URL}/${version}/${BIN_NAME}-${asset_suffix}.sha256"

log "Installing kdash ${version} for ${asset_suffix}"
log "  source: ${tarball_url}"
log "  prefix: ${INSTALL_DIR}"

mkdir -p "$INSTALL_DIR"
existing="$INSTALL_DIR/$BIN_NAME"

# Idempotence: skip the download if a binary at the install path already
# reports the requested version. Fast, reliable, and falls through to a full
# install on any error.
if [ -x "$existing" ]; then
  current_version=$("$existing" --version 2>/dev/null | head -1 | awk '{print $NF}' || true)
  if [ -n "$current_version" ] && [ "v${current_version}" = "$version" ]; then
    log "kdash ${version} already installed at ${existing} — nothing to do"
    log "Run \`${existing} --help\` to get started."
    exit 0
  fi
fi

work_dir=$(mktemp -d)
trap 'rm -rf "$work_dir"' EXIT

log "Fetching checksum..."
if ! download "$sums_url" "$work_dir/$BIN_NAME.sha256"; then
  err "could not fetch $sums_url"
  err "release may be partially uploaded; try again in a minute"
  exit 1
fi

# The sidecar is `shasum -a 256` output: "<64-hex>  <filename>". Take the first
# field as the expected digest.
expected_tarball_sum=$(awk 'NR==1 {print $1}' "$work_dir/$BIN_NAME.sha256")
case "$expected_tarball_sum" in
  *[!a-fA-F0-9]*|"")
    err "checksum file at $sums_url is malformed"
    exit 2
    ;;
esac

log "Downloading tarball..."
if ! download "$tarball_url" "$work_dir/$tarball"; then
  err "could not download $tarball_url"
  exit 1
fi

log "Verifying checksum..."
actual_tarball_sum=$(sha256_compute "$work_dir/$tarball")
# Lowercase both sides for a case-insensitive compare (shasum is lower, but be
# defensive against tooling that emits upper-case hex).
if [ "$(printf '%s' "$actual_tarball_sum" | tr 'A-F' 'a-f')" != \
     "$(printf '%s' "$expected_tarball_sum" | tr 'A-F' 'a-f')" ]; then
  err "checksum mismatch for ${tarball}"
  err "expected: ${expected_tarball_sum}"
  err "got:      ${actual_tarball_sum}"
  exit 2
fi

log "Extracting..."
# kdash tarballs contain the bare binary at the archive root.
(cd "$work_dir" && tar -xzf "$tarball") || {
  err "tar -xzf failed on $tarball"
  exit 1
}

extracted_bin="$work_dir/$BIN_NAME"
if [ ! -f "$extracted_bin" ]; then
  err "extracted tarball does not contain $BIN_NAME"
  exit 1
fi

# Atomic install: write to a temp path next to the destination, then mv.
install_tmp="${INSTALL_DIR}/.${BIN_NAME}.tmp.$$"
cp "$extracted_bin" "$install_tmp"
chmod 0755 "$install_tmp"
mv "$install_tmp" "$existing"

log ""
log "Installed kdash ${version} -> ${existing}"

# PATH hint (no mutation). Detect colon-separated $PATH membership in a way
# that works under POSIX sh; ksh/bash glob matching against ":$PATH:".
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*)
    log "Run \`${BIN_NAME} --help\` to get started."
    ;;
  *)
    log ""
    log "Note: ${INSTALL_DIR} is not on your \$PATH."
    log "Add it to your shell's startup file, e.g.:"
    log "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    log ""
    log "Then run \`${BIN_NAME} --help\` to get started."
    ;;
esac
