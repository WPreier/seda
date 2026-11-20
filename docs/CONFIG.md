# Seda – Config & Lifecycle

Die CLI lädt standardmäßig `seda.toml`; `-f/--config PATH` wählt eine andere
Datei. `--resetconfig` schreibt den eingebetteten Default, `--validate`
validiert und aktiviert ihn ohne Serverstart. Die validierte Quelle wird als
`state_dir/.seda.active.toml` gespeichert.

```toml
schema_version = 1

[server]
listen = "127.0.0.1:9080"
state_dir = "./state"
admin_socket = "./state/admin.sock"
# user = "seda"
# group = "seda"

[trust]
allowed_peers = ["127.0.0.1/32", "::1/128"]

[proxy]
protocol = "auto"       # auto | v1 | v2 | off
require_proxy = false

[embedded_docs]
enabled = true
sedat_path = "/sedat"
seda_path = "/seda"

[[sites]]
path = "/site"
root = "./site"
```

Sites werden vollständig in den RAM geladen. `max_content_bytes` begrenzt ihre
Gesamtgröße. Alle Limits müssen größer als null sein. Unbekannte Felder,
ungültige Pfade, leere Peer-Allowlist, öffentliche/Wildcard-Binds und
`require_proxy = true` zusammen mit `protocol = "off"` werden abgelehnt.

Reload erfolgt über `reload` am Unix-Admin-Socket oder SIGHUP. Kandidaten werden
vor dem Commit vollständig geprüft; bei Fehler bleiben Runtime-Config,
In-Memory-Content und aktive Kopie unverändert.
