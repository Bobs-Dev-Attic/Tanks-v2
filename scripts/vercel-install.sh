#!/usr/bin/env bash
# Install the Rust toolchain needed to build the wasm bundle on a Vercel build
# container (Amazon Linux). Used as the `installCommand` in vercel.json.
#
# Note: compiling Bevy to wasm is heavy. Building on Vercel works but is slow;
# the recommended path is the GitHub Actions workflow that builds and deploys a
# prebuilt bundle (see .github/workflows/deploy.yml).
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"

if ! command -v rustup >/dev/null 2>&1; then
  echo "==> Installing rustup…"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable --profile minimal
fi
# shellcheck disable=SC1090
source "$HOME/.cargo/env"

rustup target add wasm32-unknown-unknown

if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "==> Installing wasm-bindgen-cli…"
  cargo install wasm-bindgen-cli --version 0.2.100
fi
