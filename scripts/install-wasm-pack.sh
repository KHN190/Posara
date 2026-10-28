#!/usr/bin/env bash
set -euo pipefail

version=0.15.0
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) target=x86_64-unknown-linux-musl ;;
  Linux-aarch64|Linux-arm64) target=aarch64-unknown-linux-musl ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  Darwin-arm64|Darwin-aarch64) target=aarch64-apple-darwin ;;
  *) echo "Unsupported wasm-pack platform: $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac

archive="wasm-pack-v${version}-${target}"
staging=$(mktemp -d)
trap 'rm -rf "$staging"' EXIT
curl --fail --location --retry 3 \
  "https://github.com/wasm-bindgen/wasm-pack/releases/download/v${version}/${archive}.tar.gz" \
  --output "$staging/archive.tar.gz"
tar -xzf "$staging/archive.tar.gz" -C "$staging"
bin_dir="${CARGO_HOME:-$HOME/.cargo}/bin"
mkdir -p "$bin_dir"
# Copy rather than rename: the temporary directory may be on another filesystem.
install -m 755 "$staging/$archive/wasm-pack" "$bin_dir/wasm-pack"
if [[ -n "${GITHUB_PATH:-}" ]]; then
  printf '%s\n' "$bin_dir" >> "$GITHUB_PATH"
fi
"$bin_dir/wasm-pack" --version
