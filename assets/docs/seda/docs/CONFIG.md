# Seda – Config & Lifecycle

`seda -f PATH` wählt die User-Config. `--resetconfig` schreibt den Default,
`--validate` aktiviert eine gültige Config ohne Serverstart. Die aktive Kopie
liegt als `state_dir/.seda.active.toml`.

```toml
schema_version = 2

[server]
state_dir = "/var/lib/seda"
admin_socket = "/run/seda/admin.sock"
user = "seda"
group = "seda"

[trust]
allowed_peers = ["127.0.0.1/32", "::1/128", "192.168.36.0/24"]

[embedded_docs]
enabled = true
sedat_path = "/sedat"
seda_path = "/seda"

[proxy]
protocol = "auto" # auto | v1 | v2 | off
require_proxy = false

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

Jede Site besitzt einen Listener und eigene Roots. Globale Trust-, Proxy- und
Embedded-Docs-Werte gelten standardmäßig für alle Sites und können pro Site
überschrieben werden. Öffentliche/Wildcard-Binds, doppelte Listener, leere
Peer-Allowlist, unbekannte Felder und ungültige Site-Pfade werden abgelehnt.

Schema 1 bleibt kompatibel: `server.listen` und alte `[[sites]]` werden
in-memory zu Listener und Roots von `site1`; die Datei bleibt unverändert.
Admin-`reload` und SIGHUP behalten bei jedem Fehler Config, aktive Kopie und
Content. Listener-Änderungen benötigen einen Neustart.
