# Seda – Config & Lifecycle

Identisch zu SeDat:

- Aktive Config = versteckte Kopie
- `--resetconfig`
- Fehler mit Zeile + Grund → alte Config bleibt aktiv
- Regex-Matching
- Eingebettete 404-Seite

Zusätzlich:

```toml
[embedded_docs]
enabled = true   # Standard

[proxy]
protocol = "proxy_v2"
```
