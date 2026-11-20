# Seda – Requirements

- Rein statische HTML-Seiten ausschließlich aus dem Speicher (im Binary eingebettet)
- Kommunikation mit SeDat über PROXY Protocol v1/v2 (empfohlen)
- Eingebaute Dokumentation von SeDat und Seda unter `/sedat` und `/seda`
- Aktivierung/Deaktivierung über Config (Standard: aktiv)
- Gleiche Config-, Security- und Betriebsprinzipien wie SeDat
- Privilege Drop, Limits, non-blocking Logging, Admin-Socket, systemd
