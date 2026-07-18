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

# Optional size optimisation. Off by default to keep the deploy loop fast; set
# WASM_OPT=1 to enable. wasm-opt must be told which wasm features rustc emitted,
# and we write to a temp file so a failure never corrupts the good bundle.
if [ "${WASM_OPT:-0}" = "1" ] && command -v wasm-opt >/dev/null 2>&1; then
  echo "==> Optimising wasm with wasm-opt…"
  if wasm-opt -Os \
      --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext \
      --enable-mutable-globals --enable-multivalue --enable-reference-types \
      -o dist/tanks_bg.opt.wasm dist/tanks_bg.wasm; then
    mv dist/tanks_bg.opt.wasm dist/tanks_bg.wasm
  else
    echo "==> wasm-opt failed; keeping the unoptimised wasm"
    rm -f dist/tanks_bg.opt.wasm
  fi
else
  echo "==> Skipping wasm-opt (set WASM_OPT=1 to enable)"
fi

echo "==> Assembling static site…"
cp web/index.html dist/index.html
# Inject the version into the HTML loading screen.
sed -i "s/__GAME_VERSION__/${VERSION}/g" dist/index.html

echo "==> Done. Static site in ./dist"
ls -lh dist
