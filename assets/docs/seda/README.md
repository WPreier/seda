# Seda

**Seda** ist ein statisches In-Memory-HTTP-Backend für SeDat.

Companion-Projekt: **[SeDat](https://github.com/WPreier/sedat)** (der Reverse-Proxy).

## Verbindung zu SeDat

`site1` nutzt standardmäßig `127.0.0.1:9080`; weitere Sites können in demselben
Prozess auf zusätzlichen Ports lauschen. Trust 2B akzeptiert plain HTTP und
PROXY v1/v2 ausschließlich von `trust.allowed_peers`; Forwarded-HTTP-Header sind
keine Vertrauensquelle.

```
Internet → SeDat (Proxy) → Seda (Static Memory Client)
```

Details: [docs/CONNECTION.md](docs/CONNECTION.md)

## Inhalt und Betrieb

Die Dokumentationen unter `/sedat` und `/seda` sind eingebettet. Optionale
`[[sites.<name>.roots]]` werden pro Listener vollständig in den RAM geladen.
`seda -f PATH` wählt die
Config; Unix unterstützt fail-closed Reload über Admin-Socket und SIGHUP.
Öffentliche und Wildcard-Binds werden abgelehnt.

## Dokumentation

- [Requirements](docs/REQUIREMENTS.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Config](docs/CONFIG.md)
- [Security & Operations](docs/SECURITY_OPS.md)
- [Connection to SeDat](docs/CONNECTION.md)
