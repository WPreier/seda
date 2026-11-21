# Verbindung zwischen Seda und SeDat

SeDat übernimmt TLS, Routing und Edge-Schutz; Seda liefert statische Inhalte
aus dem RAM. Sedas `site1` erwartet den Upstream standardmäßig auf
`127.0.0.1:9080`; weitere SeDat-Upstreams können auf zusätzliche Site-Listener
desselben Seda-Prozesses zeigen.

## Kommunikationsfluss

```
Client
   ↓
SeDat (HTTPS, HTTP/1.1 + HTTP/2 + HTTP/3)
   ↓  PROXY Protocol v1/v2 oder plain HTTP (nur Allowlist-Peers)
Seda (HTTP, Seiten aus Memory)
```

## Trust 2B

Zuerst wird die echte TCP-Peer-IP gegen `trust.allowed_peers` geprüft. Nur danach
wird ein PROXY-Header ausgewertet. `protocol = "auto"` akzeptiert v1, v2 und
plain HTTP; `require_proxy = true` verbietet plain HTTP. `X-Forwarded-For`,
`Forwarded` und `X-Real-IP` sind niemals Vertrauensquellen.

## SeDat-E2E

Für Sedats `examples/e2e_seda` bzw. `mock-seda` zeigt der Backend-Upstream auf
`127.0.0.1:9080` und sendet PROXY v2. Seda kann dafür mit dem Default gestartet
werden; um den Header zwingend zu verlangen:

```toml
[sites.site1]
listen = "127.0.0.1:9080"
[trust]
allowed_peers = ["127.0.0.1/32", "::1/128"]
[proxy]
protocol = "v2"
require_proxy = true
```

Danach liefern `https://<sedat-host>/sedat` und `/seda` die eingebetteten
Demo-Dokumentationen. Direkte Smoke-Tests gegen Seda benötigen bei dieser
strikten Einstellung ebenfalls einen PROXY-v2-Preamble.
