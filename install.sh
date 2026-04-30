#!/usr/bin/env sh
set -eu

REPO="${CAPACITOR_REPO:-Ayobami-00/capacitor}"
INSTALL_DIR="${CAPACITOR_INSTALL_DIR:-$HOME/.local/bin}"
BASE_URL="https://github.com/${REPO}/releases/latest/download"

info() {
  printf '%s\n' "$1"
}

err() {
  printf 'error: %s\n' "$1" >&2
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    err "required command not found: $1"
    exit 1
  fi
}

download() {
  url="$1"
  out="$2"

  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$out"
  elif command -v wget >/dev/null 2>&1; then
    wget -q "$url" -O "$out"
  else
    err "curl or wget is required"
    exit 1
  fi
}

detect_target() {
  os="$(uname -s)"
  arch="$(uname -m)"

  case "${os}:${arch}" in
    Darwin:arm64 | Darwin:aarch64)
      printf '%s\n' "aarch64-apple-darwin"
      ;;
    Linux:x86_64 | Linux:amd64)
      printf '%s\n' "x86_64-unknown-linux-gnu"
      ;;
    Darwin:x86_64)
      err "prebuilt binaries are currently unavailable for macOS Intel"
      err "install with Cargo or build from source instead"
      exit 1
      ;;
    *)
      err "unsupported platform: ${os}/${arch}"
      err "supported targets: aarch64-apple-darwin, x86_64-unknown-linux-gnu"
      exit 1
      ;;
  esac
}

verify_checksum() {
  asset="$1"
  checksums="$2"

  if command -v sha256sum >/dev/null 2>&1; then
    (cd "$(dirname "$asset")" && grep " $(basename "$asset")\$" "$(basename "$checksums")" | sha256sum -c -)
  elif command -v shasum >/dev/null 2>&1; then
    (cd "$(dirname "$asset")" && grep " $(basename "$asset")\$" "$(basename "$checksums")" | shasum -a 256 -c -)
  else
    info "warning: no sha256sum or shasum found; skipping checksum verification"
  fi
}

target="$(detect_target)"
asset="cap-${target}.tar.gz"

need_cmd uname
need_cmd tar
need_cmd mkdir
need_cmd cp
need_cmd chmod

tmp_dir="$(mktemp -d 2>/dev/null || mktemp -d -t capacitor)"
cleanup() {
  rm -rf "$tmp_dir"
}
trap cleanup EXIT INT TERM

archive_path="${tmp_dir}/${asset}"
checksums_path="${tmp_dir}/SHA256SUMS"
unpack_dir="${tmp_dir}/unpack"

info "Installing Capacitor for ${target}"
download "${BASE_URL}/${asset}" "$archive_path"
download "${BASE_URL}/SHA256SUMS" "$checksums_path"
verify_checksum "$archive_path" "$checksums_path"

mkdir -p "$unpack_dir"
tar -xzf "$archive_path" -C "$unpack_dir"

mkdir -p "$INSTALL_DIR"
cp "$unpack_dir/cap" "$INSTALL_DIR/cap"
chmod +x "$INSTALL_DIR/cap"

info "Installed cap to ${INSTALL_DIR}/cap"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    info ""
    info "Add ${INSTALL_DIR} to your PATH to run cap from any shell."
    info "For zsh, add this to ~/.zshrc:"
    info "  export PATH=\"${INSTALL_DIR}:\$PATH\""
    ;;
esac
