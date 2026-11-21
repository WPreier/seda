# Seda – Requirements

- Hybrid-Content: Build-time eingebettete Markdown-Dokumentation plus optionale
  `[[sites.<name>.roots]]`, die pro Listener vor der Aktivierung vollständig
  in den RAM geladen werden.
- Ein Prozess bindet alle benannten Site-Listener und verwaltet sie über einen
  Admin-Socket, einen Reload und eine systemd-Unit.
- Demo-Routen `/sedat` und `/seda`; unbekannte Pfade erhalten die eingebettete
  404-Seite mit Status 404.
- Trust 2B: plain HTTP sowie PROXY Protocol v1/v2 nur von
  `trust.allowed_peers`; leere Allowlist ist ungültig. Forwarded-Header sind
  ausschließlich Log-Daten.
- Default-Upstream `127.0.0.1:9080`; öffentliche und Wildcard-Binds werden
  abgelehnt, Loopback/private/Link-Local sind erlaubt.
- Fail-closed Config-Lifecycle mit `-f/--config`, `--resetconfig`,
  `--validate`, versteckter aktiver Kopie und atomarem Reload.
- Reload über Unix-Admin-Socket oder SIGHUP; ein Fehler behält Config, aktive
  Kopie und Content vollständig bei.
- Begrenzte Verbindungen, Header, Timeouts und Content-Größe; Privilege-Drop,
  systemd notify/watchdog und Admin-Socket auf Unix.
