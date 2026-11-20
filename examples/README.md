# SeDat E2E drop-in

Seda can replace the `mock-seda` backend used by SeDat's
`examples/e2e_seda`. Start Seda on the upstream address expected by that
example:

```powershell
.\scripts\cargo-with-msvc.ps1 run -- -f .\seda.toml
```

The shipped config listens on `127.0.0.1:9080`, trusts only loopback peers and
accepts plain HTTP or PROXY v1/v2. For an E2E run that must verify SeDat's PROXY
v2 emission, use:

```toml
schema_version = 1

[server]
listen = "127.0.0.1:9080"

[trust]
allowed_peers = ["127.0.0.1/32", "::1/128"]

[proxy]
protocol = "v2"
require_proxy = true
```

Point the SeDat backend/upstream at `127.0.0.1:9080` with PROXY protocol v2
enabled, then request `/sedat`, `/seda`, and a missing path through SeDat.
Expected statuses are 200, 200, and 404. No change to the SeDat repository is
required.
