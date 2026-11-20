# SeDat – Security

## Principles

- **Fail-closed:** invalid config / untrusted PROXY / unknown Host → reject.
- **Least privilege:** bind then drop; no lasting root.
- **Explicit trust:** PROXY and admin paths require allowlists / socket ACLs.
- **Memory-safe TLS:** rustls only; TLS 1.2 and 1.3 (prefer 1.3).

## Threat model

Full write-up: **[THREAT_MODEL.md](THREAT_MODEL.md)** (assets, trust boundaries, attack IDs T1–T10, test mapping).

| Threat | Mitigation |
|--------|------------|
| Config injection / bad reload | Fail-closed; line+reason; previous active kept |
| PROXY spoofing | Outbound PROXY to Seda; inbound PROXY default off + CIDR allowlist; untrusted PROXY frames dropped |
| Domain fronting | SNI ↔ Host consistency check |
| Request smuggling | Reject CL+TE; strict header parsing; reject absolute-URI tricks; size caps |
| Slowloris / header bomb | Request timeout on connections; header count/size caps |
| Edge DoS | Token-bucket per verified client IP; connection limits |
| Privilege abuse | Drop after bind; Docker `cap_drop`, `no-new-privileges` |
| Log blocking | Async bounded channel; drop+metric on overflow |
| Cert loss on renew fail | Hot-swap; keep previous cert on ACME failure |
| Admin exposure | Unix socket only (`0600`); no public HTTP admin |
| Schema surprise upgrades | Exact `schema_version`; migrate helper fail-closed |

Automated negative tests live in `tests/attack_hygiene.rs` (PROXY injection, header bomb, smuggling, Slowloris timeout, absolute-URI). Manual scripts: `examples/hardening/`.

## TLS policy

- Protocols: **TLS 1.2 and 1.3 only** (1.3 preferred).
- No TLS 1.0/1.1, SSLv2/3.
- Certificates: ACME **DNS-01** (staging default in dev; production explicit). Local CA: **Pebble** via Compose profile `acme`.
- OCSP stapling: **not** a hard operational dependency (LE OCSP sunset).
- Port 80: HTTPS redirect only (DNS-01 does not need HTTP-01).

## PROXY Protocol

- **Outbound to Seda:** PROXY v2 recommended (v1 optional).
- **Inbound from clients/LB:** default **disabled**. Enable only with `proxy_protocol.inbound_allowlist` CIDRs. Empty allowlist + enabled → validation error. PROXY signatures from untrusted peers are **dropped**.
- Seda / Mock-Seda must listen on localhost/private net and trust only SeDat.

## Rate limits & hygiene

- Token bucket per client IP (after TLS / inbound PROXY rewrite, before expensive proxy work).
- Caps: connections, request line, header count/size, body size.
- Unknown Host: reject (default).

## Admin / metrics / health

- Control plane: Unix admin socket — `status`, `health`, `metrics`, `reload`, `cert`.
- **Never** expose SeDat metrics/health as a public HTTP control plane. Operators may scrape the admin socket from loopback.

## Logging

Non-blocking `tracing` writer with bounded channel. Overflow: drop events and increment metric — never block the request path.

## Checklist (creative security points)

- [x] Admin Unix socket (status / reload / cert / health / metrics)
- [x] Threat-model doc + attack tests
- [x] Pebble in Docker Compose (`--profile acme`)
- [x] Mock-Seda with PROXY parser + trust check
- [x] Inbound PROXY allowlist (default off, wired into accept path)
- [x] Metrics/health via admin socket only (not public control plane)
- [x] Config `schema_version` + fail-closed migration helper

## Reporting

Security issues: prefer private disclosure to the maintainer; do not open public issues with exploit details until patched.
