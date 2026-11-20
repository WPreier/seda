# Seda – Security & Operations

## Netzwerk und Trust

- Default-Bind ist `127.0.0.1:9080`. Global routbare und Wildcard-Adressen
  werden abgelehnt; private und Link-Local-Netze sind für getrennte
  Proxy-Backend-Netze erlaubt.
- TCP-Peers müssen in `trust.allowed_peers` stehen. Die Liste darf nicht leer
  sein. PROXY v1/v2 wird ausschließlich nach dieser Prüfung vertraut.
- Forwarded-HTTP-Header werden nur protokolliert, nicht authentifiziert.

## Härtung und Grenzen

Verbindungen, Headeranzahl/-größe, Request-/Idle-Zeit und geladener Site-Content
sind begrenzt. Site-Roots verbieten `..`; kanonische Pfade und Symlinks dürfen
den Root nicht verlassen. Nach dem Laden bedient der Request-Pfad keine
Dateisystemzugriffe. Auf Unix bindet Seda Listener und Admin-Socket vor dem
optionalen Group-/User-Drop.

## Betrieb

Der Unix-Admin-Socket hat Modus `0600` und unterstützt `status`, `reload`,
`help`, `quit`. SIGHUP nutzt denselben serialisierten, fail-closed Reload.
`deploy/seda.service` aktiviert systemd notify/watchdog und Härtungsoptionen.
Windows unterstützt derzeit keinen Admin-Socket, SIGHUP oder Privilege-Drop.

Die eingebetteten Demo-Pfade `/sedat` und `/seda` sind standardmäßig aktiv und
können über `[embedded_docs] enabled = false` deaktiviert werden.
