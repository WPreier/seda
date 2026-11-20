# SeDat – Threat Model

Detailed companion to the abridged table in [SECURITY.md](SECURITY.md). Fail-closed defaults are mandatory; enabling trust (inbound PROXY, admin socket access, ACME production) must be explicit.

## Assets

| Asset | Why it matters |
|-------|----------------|
| TLS private keys / ACME account | Impersonation of served names |
| Active config (`.sedat.active.toml`) | Routing, trust boundaries, limits |
| Admin Unix socket | Reload, status, cert metadata |
| Client IP integrity (rate-limit / logs / outbound PROXY) | Spoofing bypasses edge controls |
| Upstream Seda / Mock-Seda | Origin content; must not be internet-reachable |

## Trust boundaries

```text
[Internet clients] --untrusted--> (public :80/:443)
                                      |
                                   SeDat edge
                                      |
         +----------------------------+----------------------------+
         |                            |                            |
   Admin Unix socket           Outbound PROXY v2              ACME DNS-01
   (0600, local only)          to Seda (private net)          (provider API)
```

1. **Public listeners** – all clients untrusted.
2. **Admin socket** – filesystem ACL only (mode `0600`); no HTTP admin on public listeners.
3. **Inbound PROXY** – default **off**. When enabled, only allowlisted CIDRs may send PROXY; others are dropped (injection defense).
4. **Upstream** – SeDat is the sole trusted PROXY source; Mock-Seda/Seda enforce their own trust CIDRs.

## Abuse cases & tests

| ID | Attack | Mitigation | Automated coverage |
|----|--------|------------|--------------------|
| T1 | PROXY injection (spoofed client IP) | Inbound PROXY off by default; untrusted PROXY frames rejected | `tests/attack_hygiene.rs` |
| T2 | Slowloris / slow headers | Connection request timeout on accept loops | `tests/attack_hygiene.rs`, `examples/hardening/slow_read.py` |
| T3 | Header bomb | `max_header_bytes` / `max_headers` → 431 | `tests/attack_hygiene.rs`, `examples/hardening/header_flood.py` |
| T4 | Request smuggling (CL+TE) | Explicit reject; hyper parse hygiene; TE stripped on forward | `tests/attack_hygiene.rs` |
| T5 | Absolute-URI / Host mismatch | 400 Bad Request | `tests/attack_hygiene.rs` |
| T6 | Domain fronting (SNI≠Host) | 421 on TLS path | TLS handler in `src/tls` |
| T7 | Config downgrade / unknown schema | `schema_version` exact match; `migrate_toml` fail-closed | `src/config/migrate.rs` |
| T8 | Bad reload | Previous active config retained | `src/config` unit tests |
| T9 | Public metrics/admin HTTP | No built-in `/metrics`/`/health` control plane; use admin socket | `tests/attack_hygiene.rs`, admin `health`/`metrics` cmds |
| T10 | Privilege persistence | Drop after bind (Linux); Docker `cap_drop` | `src/hardening` |

## PROXY Protocol policy (detail)

- **Outbound (to Seda):** `proxy_protocol.outbound = "v2"` recommended.
- **Inbound (from LB/clients):** `inbound_enabled = false` by default.
  - If a PROXY v1/v2 signature is seen while disabled or from a non-allowlisted peer → **connection dropped** (fail-closed).
  - Enabling inbound without CIDRs fails validation.
- **Mock-Seda** (`deploy/mock-seda/server.py`) independently rejects PROXY from non-`TRUST_CIDRS` peers with HTTP 403.

## Metrics & health

| Channel | Allowed |
|---------|---------|
| Admin socket commands `health`, `metrics`, `status` | Yes |
| Loopback-only sidecar scraping the admin socket | Yes (operator-provided) |
| Public HTTP `/metrics`, `/health` as SeDat control plane | **No** |

## Schema upgrades

See [CONFIG.md](CONFIG.md) and `sedat --migrate-config`. Unknown `schema_version` values refuse to load.

## Residual risks / non-goals

- Landlock/seccomp: deferred ([IDEAS.md](IDEAS.md)).
- Full HTTP/2 / HPACK bomb corpus: rely on hyper + size caps; extend tests as needed.
- Pebble E2E issuance in CI: Compose profile `acme` is manual/dev; unit coverage checks the `pebble` directory alias.
