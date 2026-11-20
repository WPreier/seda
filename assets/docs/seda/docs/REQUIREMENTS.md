# Seda – Requirements

- Hybrid-Content: eingebettete Dokumentation plus vollständig in den RAM
  geladene optionale Site-Roots.
- Demo-Routen `/sedat` und `/seda` sowie eingebettete HTTP-404-Seite.
- Trust 2B: plain HTTP und PROXY v1/v2 nur von Allowlist-Peers; Forwarded-Header
  werden nicht vertraut.
- Default `127.0.0.1:9080`; keine öffentlichen oder Wildcard-Binds.
- Fail-closed Config und Reload mit `-f`, aktiver Kopie, Admin-Socket und SIGHUP.
- Ressourcenlimits, Unix-Privilege-Drop und systemd notify/watchdog.
