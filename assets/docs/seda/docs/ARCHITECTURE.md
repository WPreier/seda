# Seda – Architecture

Seda ist ein begrenztes HTTP/1.1-In-Memory-Backend für SeDat.

- Der Transport-Peer wird vor dem PROXY-v1/v2-Parser gegen die Allowlist geprüft.
- Build-time Markdown-HTML und 404 sind eingebettet.
- Optionale Sites werden vor Aktivierung vollständig materialisiert und danach
  ohne Dateisystemzugriff ausgeliefert.
- Reload bereitet Config und Content vor und tauscht beide erst nach vollständigem
  Erfolg; Fehler behalten den alten Zustand.
