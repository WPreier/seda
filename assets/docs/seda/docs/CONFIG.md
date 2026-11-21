# Seda – Config & Lifecycle

`seda -f PATH` wählt die User-Config. `--resetconfig` schreibt den Default,
`--validate` aktiviert eine gültige Config ohne Serverstart. Die aktive Kopie
liegt als `state_dir/.seda.active.toml`.

```toml
schema_version = 2

[server]
state_dir = "./state"
admin_socket = "./state/admin.sock"

[trust]
allowed_peers = ["127.0.0.1/32", "::1/128"]

[embedded_docs]
enabled = true

[proxy]
protocol = "auto" # auto | v1 | v2 | off
require_proxy = false

[sites.site1]
listen = "127.0.0.1:9080"
[[sites.site1.roots]]
path = "/"
root = "./site1"

[sites.site2]
listen = "127.0.0.1:9081"
```

Jede Site besitzt einen Listener und eigene Roots. Globale Trust-, Proxy- und
Embedded-Docs-Werte gelten standardmäßig für alle Sites und können pro Site
überschrieben werden. Öffentliche/Wildcard-Binds, doppelte Listener, leere
Peer-Allowlist, unbekannte Felder und ungültige Site-Pfade werden abgelehnt.

Schema 1 bleibt kompatibel: `server.listen` und alte `[[sites]]` werden
in-memory zu Listener und Roots von `site1`; die Datei bleibt unverändert.
Admin-`reload` und SIGHUP behalten bei jedem Fehler Config, aktive Kopie und
Content. Listener-Änderungen benötigen einen Neustart.
