#!/bin/bash

# The install script is licensed under the CC-0 1.0 license.

# See https://github.com/kdash-rs/kdash/blob/main/LICENSE for more details.
#
# To run this script execute:
#   `curl https://raw.githubusercontent.com/kdash-rs/kdash/main/deployment/getLatest.sh | sh`

GITHUB_REPO="kdash"
GITHUB_USER="kdash-rs"
EXE_FILENAME="kdash"
EXE_DEST_DIR="/usr/local/bin"

bye() {
  result=$?
  if [ "$result" != "0" ]; then
    echo "Fail to install ${GITHUB_USER}/${GITHUB_REPO}"
  fi
  exit $result
}

fail() {
  echo "$1"
  exit 1
}

find_download_url() {
  local SUFFIX=$1
  local LATEST_URL="https://api.github.com/repos/${GITHUB_USER}/${GITHUB_REPO}/releases/latest"
  local URL=$(curl -s "${LATEST_URL}" | grep "browser_download_url.*${SUFFIX}" | cut -d : -f 2,3 | tr -d \" | head -n 1)
  echo "${URL//[[:space:]]/}"
}

find_arch() {
  local ARCH=$(uname -m)
  case $ARCH in
  armv5*) ARCH="armv5" ;;
  armv6*) ARCH="armv6" ;;
  armv7*) ARCH="armv7" ;;
  arm64) ARCH="aarch64" ;;
  aarch64) ARCH="aarch64" ;;
  x86) ARCH="386" ;;
  # x86_64) ARCH="amd64";;
  i686) ARCH="386" ;;
  i386) ARCH="386" ;;
  esac
  echo $ARCH
}

find_os() {
  local OS=$(echo $(uname) | tr '[:upper:]' '[:lower:]')

  case "$OS" in
  # Minimalist GNU for Windows
  mingw*) OS='windows' ;;
  msys*) OS='windows' ;;
  esac
  echo $OS
}

find_suffix() {
  local ARCH=$1
  local OS=$2
  local SUFFIX="$OS.tar.gz"
#   case "$OS" in
#   "linux") SUFFIX='linux-musl.tar.gz' ;;
#   "darwin") SUFFIX='macos.tar.gz' ;;
#   "windows") SUFFIX='windows.tar.gz';;
#   esac
  case "$ARCH" in
  "aarch64") SUFFIX="aarch64-gnu.tar.gz" ;;
  "arm64") SUFFIX="aarch64-gnu.tar.gz" ;;
  esac
  echo $SUFFIX
}

download_file() {
  local FILE_URL="$1"
  local FILE_PATH="$2"
  echo "Getting $FILE_URL ....."
  httpStatusCode=$(curl -s -w '%{http_code}' -L "$FILE_URL" -o "$FILE_PATH")
  if [ "$httpStatusCode" != 200 ]; then
    echo "failed to download '${URL}'"
    fail "Request fail with http status code $httpStatusCode"
  fi
}

find_exec_dest_path() {
  if [ ! -w $EXE_DEST_DIR ]; then
    echo "Cannot write to ${EXE_DEST_DIR}. Run with 'sudo' to install to ${EXE_DEST_DIR}. Installing to current directory now ....."
    EXE_DEST_DIR=$(pwd)
  fi
}

install_file() {
  local FILE_PATH=$1
  local EXE_DEST_FILE=$2
  TMP_DIR="/tmp/${GITHUB_USER}_${GITHUB_REPO}"
  mkdir -p "$TMP_DIR" || true
  tar xf "$FILE_PATH" -C "$TMP_DIR"
  cp "$TMP_DIR/${EXE_FILENAME}" "${EXE_DEST_FILE}"
  chmod +x "${EXE_DEST_FILE}"
  rm -rf "$TMP_DIR"
}

main() {
  find_exec_dest_path
  local EXE_DEST_FILE="${EXE_DEST_DIR}/${EXE_FILENAME}"
  local ARCH=$(find_arch)
  local OS=$(find_os)
  local SUFFIX=$(find_suffix $ARCH $OS)
  local FILE_URL=$(find_download_url $SUFFIX)
  if [ -z "${FILE_URL}" ]; then
    fail "Did not find a latest release for your system: $OS $ARCH ($SUFFIX)"
  fi
  local FILE_PATH="/tmp/${GITHUB_USER}-${GITHUB_REPO}-latest-${SUFFIX}"
  download_file "${FILE_URL}" "${FILE_PATH}"
  install_file "${FILE_PATH}" "${EXE_DEST_FILE}"
  rm -Rf ${FILE_PATH}
  echo "executable installed at ${EXE_DEST_FILE}"
  bye
}

#TODO check bash is used `readlink /proc/$$/exe`
# because the script is not compatible with dash (default sh on ubuntu), other posix only shell,...

#Stop execution on any error
trap "bye" EXIT
set -e
# set -x
main
