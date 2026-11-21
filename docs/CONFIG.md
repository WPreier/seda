# Seda – Config & Lifecycle

Die CLI lädt standardmäßig `seda.toml`; `-f/--config PATH` wählt eine andere
Datei. `--resetconfig` schreibt den eingebetteten Default, `--validate`
validiert und aktiviert ihn ohne Serverstart. Die validierte Quelle wird als
`state_dir/.seda.active.toml` gespeichert.

```toml
schema_version = 2

[server]
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

[sites.site1]
listen = "127.0.0.1:9080"
[[sites.site1.roots]]
path = "/"
root = "./site1"

[sites.site2]
listen = "127.0.0.1:9081"
[[sites.site2.roots]]
path = "/"
root = "./site2"
```

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
