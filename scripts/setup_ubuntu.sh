#!/usr/bin/env bash
set -euo pipefail

sudo apt update
sudo apt install -y build-essential curl wget default-jre tmux htop pkg-config

if ! command -v rustup >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi

if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

rustup default stable
rustup component add rustfmt clippy

cargo --version
rustc --version
