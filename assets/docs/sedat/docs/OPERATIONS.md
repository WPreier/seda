# SeDat – Operations

## Platforms

| Environment | Support |
|-------------|---------|
| Linux (native / systemd) | **Primary production** |
| WSL2 + Docker | Development & CI parity |
| Windows native | Build/test best-effort; **no** privilege-drop / systemd |

See [DEV_SETUP.md](DEV_SETUP.md).

## Quick start (dev)

```bash
cargo build
cargo run -- --config examples/config/good.toml --listen 127.0.0.1:8080
```

Docker:

```bash
docker compose -f docker-compose.dev.yml up --build
```

## Config paths

- User config: `--config PATH` (editable).
- Active copy: `.sedat.active.toml` next to state dir (mode `0600`).
- Reset: `sedat --resetconfig --config PATH`.

## Listeners

- HTTP (redirect): default `:80` in production sketches.
- HTTPS: `:443` (h1/h2); optional UDP `:443` for HTTP/3 when `experimental.h3 = true`.
- Admin: Unix socket path from config (e.g. `/run/sedat/admin.sock`).

## systemd (sketch)

Unit file: [`deploy/sedat.service`](../deploy/sedat.service).

- `Type=notify`, `WatchdogSec=` set.
- Start as root (or with `CAP_NET_BIND_SERVICE`), drop via SeDat config `user`/`group`.
- `ProtectSystem=strict`, `PrivateTmp=true`, `NoNewPrivileges=true` recommended.

## Docker hardening

See [`deploy/Dockerfile`](../deploy/Dockerfile) and compose:

- `USER` non-root after image setup
- `cap_drop: [ALL]`
- `security_opt: [no-new-privileges:true]`
- read-only rootfs + writable state volume

## ACME

- Dev/staging endpoint by default; **Pebble** alias `acme.directory = "pebble"` for local CA tests.
- Production: set `acme.directory = "production"` explicitly.
- DNS-01 provider trait; file/manual + Cloudflare stub.
- Account JSON persisted under state dir.
- Renewal ~30 days before expiry; on failure previous cert stays active.

```bash
docker compose -f docker-compose.dev.yml --profile acme up pebble dns-mock
# sedat.toml: acme.directory = "pebble", dns_provider = "file"
```

See `examples/acme_staging/README.md`.

## Admin

Unix-domain socket only (mode `0600` after bind). No public HTTP admin UI.

```bash
# status / health / metrics / reload / cert (Linux)
echo status  | socat - UNIX-CONNECT:/run/sedat/admin.sock
echo health  | socat - UNIX-CONNECT:/run/sedat/admin.sock
echo metrics | socat - UNIX-CONNECT:/run/sedat/admin.sock
echo reload  | socat - UNIX-CONNECT:/run/sedat/admin.sock
echo cert    | socat - UNIX-CONNECT:/run/sedat/admin.sock
```

## Health / metrics

Expose SeDat control-plane health/metrics **only** via the admin socket (or an operator sidecar that talks to that socket on loopback). There is no built-in public HTTP `/metrics` or `/health` control endpoint.

## Firewall

- Allow TCP 80/443 (and UDP 443 if HTTP/3 enabled).
- Block direct access to Seda from the internet.

## Release notes

Version via Cargo/`--version`. Optional SBOM via `cargo cyclonedx` (documented in Phase 8). Checklist: [phases/P08_e2e_release.md](phases/P08_e2e_release.md).
