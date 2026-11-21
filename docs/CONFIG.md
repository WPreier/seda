# Seda – Config & Lifecycle

Die CLI lädt standardmäßig `seda.toml`; `-f/--config PATH` wählt eine andere
Datei. `--resetconfig` schreibt den eingebetteten Default, `--validate`
validiert und aktiviert ihn ohne Serverstart. Die validierte Quelle wird als
`state_dir/.seda.active.toml` gespeichert.

```toml
schema_version = 2

[server]
state_dir = "/var/lib/seda"
admin_socket = "/run/seda/admin.sock"
user = "seda"
group = "seda"

[trust]
allowed_peers = ["127.0.0.1/32", "::1/128", "192.168.36.0/24"]

[proxy]
protocol = "auto"       # auto | v1 | v2 | off
require_proxy = false

[embedded_docs]
enabled = true
sedat_path = "/sedat"
seda_path = "/seda"

[sites.site1]
listen = "127.0.0.1:8001"
[[sites.site1.roots]]
path = "/malama"
root = "/opt/malama/theweb"

[sites.site2]
listen = "127.0.0.1:8002"
[[sites.site2.roots]]
path = "/ppc"
root = "/opt/ppc/theweb"

[limits]
max_connections = 1024
max_header_bytes = 16384
max_headers = 100
request_timeout_secs = 30
idle_timeout_secs = 60
max_content_bytes = 67108864

[logging]
level = "info"
```

Eine benannte Site ist eine normale Tabelle: `[sites.site1]`, **nicht**
`[[sites.site1]]`. Nur die wiederholbaren Roots verwenden
`[[sites.site1.roots]]`. Roots für `site2` müssen unter
`[[sites.site2.roots]]` stehen und dürfen nicht versehentlich erneut `site1`
zugeordnet werden. Das Trust-Feld heißt `allowed_peers`.

Jede benannte Site besitzt einen Listener und eigene `roots`, die vollständig
in den RAM geladen werden. Globale Werte für `trust`, `proxy` und
`embedded_docs` gelten als Defaults; gleichnamige Tabellen unter
`sites.<name>` überschreiben sie vollständig für diese Site. Aktivierte
eingebettete Dokumentation ist damit standardmäßig auf jedem Listener
verfügbar. `max_content_bytes` gilt pro Site. Alle Limits müssen größer als
null sein. Leere Site-Maps, doppelte Listen-Adressen, unbekannte Felder,
ungültige Pfade, leere Peer-Allowlist, öffentliche/Wildcard-Binds und
`require_proxy = true` zusammen mit `protocol = "off"` werden abgelehnt.

Schema 1 bleibt lesbar: `server.listen` wird in-memory zu
`sites.site1.listen`, alte `[[sites]]` werden zu `sites.site1.roots`.
Globale Trust-, Proxy-, Embedded-Docs- und Limit-Werte bleiben erhalten. Die
Benutzerdatei wird dabei nicht umgeschrieben; die normalisierte Runtime-Config
hat Schema 2.

Reload erfolgt über `reload` am Unix-Admin-Socket oder SIGHUP. Kandidaten werden
vor dem Commit vollständig geprüft; bei Fehler bleiben Runtime-Config,
In-Memory-Content und aktive Kopie unverändert. Content und Policies können
reloadet werden. Eine Änderung von Site-Namen oder Listen-Adressen erfordert
einen Neustart, damit weiterhin alle Listener gemeinsam oder gar nicht binden.
