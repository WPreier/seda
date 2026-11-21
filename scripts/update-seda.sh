#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"
cd /usr/local/src/seda

git pull --ff-only
cargo build --locked --release
sudo ./target/release/seda --config /etc/seda/seda.toml --validate
sudo install -o root -g root -m 0755 target/release/seda /usr/local/bin/seda
sudo systemctl restart seda
/usr/local/bin/seda --version
sudo systemctl status seda --no-pager
