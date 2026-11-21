# Seda – Architecture

Seda ist ein synchrones, begrenztes HTTP/1.1-Backend für SeDat:

1. Alle benannten Site-Listener binden nur Loopback-, private oder
   Link-Local-Adressen; der Start bindet alle oder schlägt vollständig fehl.
2. Die Transport-Peer-IP muss in `trust.allowed_peers` liegen.
3. Ein begrenzter Parser erkennt plain HTTP, PROXY v1 oder PROXY v2 gemäß
   `proxy.protocol` und `require_proxy`.
4. Ein begrenzter HTTP/1.1-Parser akzeptiert GET/HEAD und liefert Content aus
   unveränderlichen In-Memory-Maps; jede Verbindung beantwortet eine Anfrage.

## Hybrid-Content

`build.rs` konvertiert die Markdown-Bäume von Seda und SeDat zu HTML und bettet
sie zusammen mit der 404-Seite in das Binary ein. Optionale
`[[sites.<name>.roots]]` werden pro Listener
beim Start vollständig gelesen, auf Root-/Symlink-Escapes und Gesamtgröße
geprüft und anschließend ohne Dateisystemzugriff ausgeliefert. Der längste
Mount-Präfix gewinnt; ein Site-Mount kann `/` sein.

## Atomarer Reload

Reloads werden serialisiert. Zuerst werden Config und alle Sites als Kandidat
validiert/materialisiert. Erst danach werden aktive Config-Kopie, Content und
Runtime-Config ersetzt. Parse-, Validierungs- oder Content-Fehler lassen den
vorherigen Zustand aktiv. Unix bietet dafür Admin-Socket und SIGHUP.
