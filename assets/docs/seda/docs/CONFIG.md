# Seda – Config & Lifecycle

`seda -f PATH` wählt die User-Config. `--resetconfig` schreibt den Default,
`--validate` aktiviert eine gültige Config ohne Serverstart. Die aktive Kopie
liegt als `state_dir/.seda.active.toml`.

```toml
[server]
listen = "127.0.0.1:9080"

[trust]
allowed_peers = ["127.0.0.1/32", "::1/128"]

[embedded_docs]
enabled = true

[proxy]
protocol = "auto" # auto | v1 | v2 | off
require_proxy = false
```

Öffentliche/Wildcard-Binds, leere Peer-Allowlist, unbekannte Felder und
ungültige Site-Pfade werden abgelehnt. Admin-`reload` und SIGHUP behalten bei
jedem Fehler Config, aktive Kopie und Content.
