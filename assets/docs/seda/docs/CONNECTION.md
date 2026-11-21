# Verbindung zwischen Seda und SeDat

SeDat übernimmt TLS und Edge-Schutz; Seda liefert statische Inhalte aus dem RAM.

## Kommunikationsfluss

```
Client
   ↓
SeDat (HTTPS, HTTP/1.1 + HTTP/2 + HTTP/3)
   ↓  PROXY Protocol v1/v2 oder plain HTTP
Seda (HTTP, Seiten aus Memory)
```

## Trust und E2E

Sedas `site1` lauscht standardmäßig auf `127.0.0.1:9080`; weitere Sites können
weitere Upstreams bedienen. Nur
`trust.allowed_peers` dürfen verbinden. Für Sedats `examples/e2e_seda` oder
`mock-seda` zeigt der Upstream auf diesen Port und sendet PROXY v2. Mit
`protocol = "v2"` und `require_proxy = true` lässt sich dessen Emission
erzwingen. `/sedat`, `/seda` und ein fehlender Pfad ergeben 200, 200 und 404.
