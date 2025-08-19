#!/usr/bin/env sh
# Minimal installer for "av" — downloads the correct binary from GitHub Releases
# Inspired by uv's installer style: https://astral.sh/uv/install.sh

set -eu

# ----------------------- Configurable knobs -----------------------
# Override via env when needed
AV_REPO_SLUG="${AV_INSTALL_REPO:-${AV_REPO_SLUG:-guang/av}}"  # owner/repo
AV_VERSION="${AV_VERSION:-latest}"                             # e.g. v0.1.0 or latest
AV_BIN_NAME="av"
AV_TOKEN="${AV_GITHUB_TOKEN:-${GITHUB_TOKEN:-}}"              # optional for private rate limits

# Choose install dir: AV_INSTALL_DIR > ~/.local/bin > /usr/local/bin (if writable)
DEFAULT_LOCAL_BIN="$HOME/.local/bin"
AV_INSTALL_DIR="${AV_INSTALL_DIR:-}"

# ----------------------- Utils -----------------------
say() { printf "%s\n" "$*"; }
err() { printf "ERROR: %s\n" "$*" >&2; exit 1; }

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || err "need '$1' (command not found)"
}

downloader() {
  url="$1"; out="$2"
  if command -v curl >/dev/null 2>&1; then
    if [ -n "$AV_TOKEN" ]; then
      curl -fsSL -H "Authorization: Bearer $AV_TOKEN" "$url" -o "$out"
    else
      curl -fsSL "$url" -o "$out"
    fi
  elif command -v wget >/dev/null 2>&1; then
    if [ -n "$AV_TOKEN" ]; then
      wget --header "Authorization: Bearer $AV_TOKEN" -q "$url" -O "$out"
    else
      wget -q "$url" -O "$out"
    fi
  else
    err "need 'curl' or 'wget' to download"
  fi
}

detect_os() {
  case "$(uname -s)" in
    Linux) echo linux ;;
    Darwin) echo macos ;;
    *) err "unsupported OS: $(uname -s)" ;;
  esac
}

detect_arch() {
  # normalize common arch names
  arch=$(uname -m)
  case "$arch" in
    x86_64|amd64) echo x86_64 ;;
    arm64|aarch64) echo aarch64 ;;
    *) err "unsupported CPU arch: $arch" ;;
  esac
}

target_triple() {
  os="$1"; arch="$2"
  case "$os" in
    linux)
      if [ "$arch" = "x86_64" ]; then echo "x86_64-unknown-linux-gnu"; else err "no binary for $arch on Linux"; fi ;;
    macos)
      if [ "$arch" = "aarch64" ]; then echo "aarch64-apple-darwin"; else err "no binary for $arch on macOS"; fi ;;
    *) err "unsupported combo" ;;
  esac
}

pick_install_dir() {
  if [ -n "$AV_INSTALL_DIR" ]; then echo "$AV_INSTALL_DIR"; return; fi
  if [ -d "$DEFAULT_LOCAL_BIN" ] || mkdir -p "$DEFAULT_LOCAL_BIN" 2>/dev/null; then
    echo "$DEFAULT_LOCAL_BIN"; return
  fi
  if [ -w "/usr/local/bin" ]; then echo "/usr/local/bin"; return; fi
  # fallback to local bin even if not in PATH
  echo "$DEFAULT_LOCAL_BIN"
}

latest_tag() {
  # use GitHub latest redirect
  # e.g. https://github.com/<slug>/releases/latest -> 302 to .../tag/vX.Y.Z
  need_cmd curl
  url="https://github.com/$AV_REPO_SLUG/releases/latest"
  tag=$(curl -fsSLI "$url" | awk -F '/tag/' '/^location:/I{print $2}' | tr -d '\r\n') || tag=""
  if [ -z "$tag" ]; then err "failed to resolve latest tag"; fi
  # ensure tag starts with 'v'
  case "$tag" in v*) echo "$tag" ;; *) echo "v$tag" ;; esac
}

main() {
  os=$(detect_os)
  arch=$(detect_arch)
  triple=$(target_triple "$os" "$arch")

  ver="$AV_VERSION"
  if [ "$ver" = "latest" ]; then ver=$(latest_tag); fi

  asset="${AV_BIN_NAME}-${ver}-${triple}.tar.gz"

  if [ "$AV_VERSION" = "latest" ]; then
    base="https://github.com/${AV_REPO_SLUG}/releases/latest/download"
  else
    base="https://github.com/${AV_REPO_SLUG}/releases/download/${ver}"
  fi
  url="$base/$asset"

  tmpdir=$(mktemp -d 2>/dev/null || mktemp -d -t av-install)
  trap 'rm -rf "$tmpdir"' EXIT INT TERM

  say "Downloading $asset ..."
  downloader "$url" "$tmpdir/$asset" || err "failed to download asset: $url"

  say "Unpacking ..."
  tar -C "$tmpdir" -xzf "$tmpdir/$asset" || err "failed to unpack"

  bin_src="$tmpdir/$AV_BIN_NAME"
  [ -x "$bin_src" ] || err "binary not found in archive: $AV_BIN_NAME"

  inst_dir=$(pick_install_dir)
  mkdir -p "$inst_dir"
  bin_dst="$inst_dir/$AV_BIN_NAME"

  # Try to move; fall back to copy if cross-device
  if mv "$bin_src" "$bin_dst" 2>/dev/null; then : ; else cp "$bin_src" "$bin_dst"; fi
  chmod +x "$bin_dst"

  say "Installed to: $bin_dst"
  # PATH hint
  case ":$PATH:" in
    *:"$inst_dir":*) :;;
    *) say "NOTE: $inst_dir is not in PATH. Add it, e.g.:"; say "  export PATH=\"$inst_dir:\$PATH\"";;
  esac

  # Smoke test
  if "$bin_dst" --version >/dev/null 2>&1; then
    say "Run: av --help"
  fi
}

main "$@"


