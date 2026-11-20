# Seda

**Seda** (türkisch/persisch: Echo / Stimme) – Proxy-Client für rein statische HTML-Seiten, die ausschließlich aus dem Speicher ausgeliefert werden.

Companion-Projekt: **[SeDat](https://github.com/WPreier/sedat)** (der Reverse-Proxy).

## Verbindung zu SeDat

Seda wird typischerweise hinter SeDat betrieben und liefert die statischen Seiten.

```
Internet → SeDat (Proxy) → Seda (Static Memory Client)
```

Details: [docs/CONNECTION.md](docs/CONNECTION.md)

## Status

Spezifikationsphase. Vollständige Anforderungen für Code-Generierung vorhanden.

## Dokumentation

- [Requirements](docs/REQUIREMENTS.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Config](docs/CONFIG.md)
- [Security & Operations](docs/SECURITY_OPS.md)
- [Connection to SeDat](docs/CONNECTION.md)
