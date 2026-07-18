#!/usr/bin/env bash
# Build the web (wasm) bundle into ./dist, ready to deploy as static files.
#
# Requires: rustup with the wasm32-unknown-unknown target, and wasm-bindgen-cli
# matching the wasm-bindgen crate version pinned in Cargo.toml (0.2.100).
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.100
set -euo pipefail

cd "$(dirname "$0")"

# Make sure a rustup-installed toolchain (e.g. on a CI or Vercel builder) is on
# PATH.
export PATH="$HOME/.cargo/bin:$PATH"

# Single source of truth for the version: the Cargo package version.
VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"
echo "==> Building Tanks-v2 v${VERSION} for the web"

echo "==> Compiling wasm (release)…"
cargo build --release --target wasm32-unknown-unknown

echo "==> Running wasm-bindgen…"
rm -rf dist
mkdir -p dist
wasm-bindgen \
  --no-typescript \
  --target web \
  --out-dir dist \
  --out-name tanks \
  target/wasm32-unknown-unknown/release/tanks-v2.wasm

# Optional size optimisation if wasm-opt (binaryen) is available.
if command -v wasm-opt >/dev/null 2>&1; then
  echo "==> Optimising wasm with wasm-opt…"
  wasm-opt -Os -o dist/tanks_bg.wasm dist/tanks_bg.wasm
else
  echo "==> wasm-opt not found; skipping (install binaryen for a smaller build)"
fi

echo "==> Assembling static site…"
cp web/index.html dist/index.html
# Inject the version into the HTML loading screen.
sed -i "s/__GAME_VERSION__/${VERSION}/g" dist/index.html

echo "==> Done. Static site in ./dist"
ls -lh dist
