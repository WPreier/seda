# SeDat – Ideas & open questions

Items here are **not** blockers. Triaged in Phase 8.

## Adopted into plan (implemented)

- Admin Unix socket (not public HTTP admin) — `src/admin`
- Threat-model doc + negative tests — `docs/THREAT_MODEL.md`, `tests/attack_hygiene.rs`
- Pebble + Mock-DNS for ACME — Compose profile `acme`
- Mock-Seda with PROXY trust check — `deploy/mock-seda`
- Inbound PROXY allowlist (default off) — wired in accept path
- Metrics/health only on admin/loopback — admin `health`/`metrics`
- Config schema version + migration path — `config::migrate`, `--migrate-config`

## Open / later

| Idea | Notes | Triage |
|------|-------|--------|
| Landlock / seccomp profiles | After privilege drop; Linux-only | Phase 8+: investigate |
| ARI (ACME Renewal Info) | Prefer when CA supports | Nice-to-have |
| Multiple DNS providers | Trait ready; Cloudflare first | Extend as needed |
| Metrics export (Prometheus) | Only via admin/loopback | Deferred |
| Hot-reload without process restart | In-process reload of routes/certs | Partial (config + certs) |
| Windows service wrapper | Out of scope; use WSL/Docker | Won't do for prod |
| Pingora backend option | Explicitly rejected for MVP | Won't do |
| OCSP stapling dependency | Avoid; LE OCSP sunset | Won't do as hard req |

## Spec gaps resolved in Phase 0

- Stack choice: hyper/tokio/rustls (not Pingora)
- Primary platform: Linux; Windows via WSL/Docker
- PROXY trust model documented
