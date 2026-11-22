#!/usr/bin/env bash
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"
cd /usr/local/src/seda

git pull --ff-only
cargo build --locked --release
sudo ./target/release/seda --config /etc/seda/seda.toml --validate
sudo install -o root -g root -m 0755 target/release/seda /usr/local/bin/seda

unit_source=deploy/seda.service
unit_target=/etc/systemd/system/seda.service
if ! sudo cmp -s "$unit_source" "$unit_target"; then
  sudo install -o root -g root -m 0644 "$unit_source" "$unit_target"
  sudo systemctl daemon-reload
fi

sudo systemctl restart seda
/usr/local/bin/seda --version
sudo systemctl status seda --no-pager
