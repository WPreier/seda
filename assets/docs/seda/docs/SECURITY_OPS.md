# Seda – Security & Operations

- Default-Bind `127.0.0.1:9080`; nur Loopback, private und Link-Local-Adressen.
- TCP-Peer-Allowlist vor jeder Auswertung von PROXY v1/v2.
- Begrenzte Verbindungen, Header, Timeouts und Content-Größe.
- Site-Root- und Symlink-Escape-Prüfung; Request-Auslieferung ohne Dateisystem.
- Unix: optionaler Group-/User-Drop, Admin-Socket `0600`, SIGHUP sowie systemd
  notify/watchdog.

Windows hat derzeit keinen Admin-Socket, SIGHUP oder Privilege-Drop.
Die eingebetteten `/sedat`- und `/seda`-Pfade sind konfigurierbar.
