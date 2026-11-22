# Seda

**Seda** (türkisch/persisch: Echo / Stimme) ist ein statisches In-Memory-HTTP-Backend für SeDat.

Companion-Projekt: **[SeDat](https://github.com/WPreier/sedat)** (der Reverse-Proxy).

Die aktuelle Version ist **0.2.3** mit Konfigurationsschema **2**. Sie steht in
`Cargo.toml` und folgt [Semantic Versioning](https://semver.org/)
(`MAJOR.MINOR.PATCH`): Neue Funktionen oder Schema-Erweiterungen erhöhen
`MINOR`; reine Dokumentationskorrekturen in einem späteren Release erhöhen
`PATCH`. `seda --version` verwendet dieselbe Paketversion.

## Verbindung zu SeDat

Seda kann in einem Prozess auf mehreren unter `sites` konfigurierten Ports
lauschen; `site1` verwendet standardmäßig `127.0.0.1:9080`. Verbindungen sind
nur von `trust.allowed_peers` erlaubt. Von diesen Peers akzeptiert Trust-Modell 2B
wahlweise PROXY v1/v2 oder plain HTTP (`proxy.protocol = "auto"`). Forwarded
HTTP-Header bestimmen niemals die Client-Identität.

```
Internet → SeDat (Proxy) → Seda (Static Memory Client)
```

Details: [docs/CONNECTION.md](docs/CONNECTION.md)

## Inhalt

Die SeDat- und Seda-Dokumentation wird beim Build nach HTML konvertiert und unter
`/sedat` und `/seda` eingebettet und standardmäßig auf jedem Site-Listener
angeboten. Zusätzliche `[[sites.<name>.roots]]` werden beim Start oder Reload
vollständig aus einem Dateisystem-Root in den RAM der jeweiligen Site geladen;
Requests lesen danach nicht vom Dateisystem. Eine eingebettete Seite beantwortet
unbekannte Pfade mit HTTP 404.

## Start, Reload und systemd

`seda -f /etc/seda/seda.toml` wählt die Konfiguration. `--resetconfig` erzeugt
die eingebettete Standardkonfiguration, `--validate` aktiviert eine gültige
Konfiguration und beendet das Programm. Öffentliche und Wildcard-Bind-Adressen
werden abgelehnt; erlaubt sind Loopback-, private und Link-Local-Adressen.

Alle Listener teilen sich einen Prozess, einen Reload und eine systemd-Unit.
Unter Unix lauscht Seda am konfigurierten `server.admin_socket` (Modus `0600`).
Die Befehle sind `status`, `reload`, `help` und `quit`; alternativ löst `SIGHUP`
denselben fail-closed Reload aus. Eine gehärtete Notify/Watchdog-Unit liegt unter
[`deploy/seda.service`](deploy/seda.service).

Unter Windows werden HTTP und Config unterstützt; Unix-Admin-Socket, SIGHUP,
Privilege-Drop und systemd-Integration sind dort nicht verfügbar.

## Entwicklung

`cargo test` führt die Suite aus. Falls Cargo in einer normalen PowerShell die
MSVC/Windows-SDK-Bibliotheken nicht findet:

```powershell
.\scripts\cargo-with-msvc.ps1 test
```

## Dokumentation

- [Linux-Installation (Ubuntu, Debian, AlmaLinux)](docs/INSTALL.md)
- [Requirements](docs/REQUIREMENTS.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Config](docs/CONFIG.md)
- [Security & Operations](docs/SECURITY_OPS.md)
- [Connection to SeDat](docs/CONNECTION.md)
