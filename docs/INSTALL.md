# Seda unter Linux installieren

Diese Anleitung gilt für Ubuntu, Debian und AlmaLinux. Seda ist ein internes
HTTP-Backend: Es darf nicht direkt aus dem Internet erreichbar sein. SeDat,
Apache oder Nginx übernimmt die öffentliche Edge-/TLS-Funktion.

Die folgenden Schritte sind für eine frisch installierte VM gedacht und können
von oben nach unten ausgeführt werden. Die Befehle verwenden `sudo`. Bei einer
reinen Debian-Minimalinstallation ist `sudo` möglicherweise noch nicht
vorhanden; dann zunächst mit `su -` eine Root-Shell öffnen, `sudo` installieren
oder in dieser Anleitung `sudo` weglassen.

## 1. Basispakete installieren

Seda wird derzeit aus dem Quellcode gebaut. Dafür werden Git, ein
C/C++-Build-Werkzeugsatz, `curl`, CA-Zertifikate und Rust benötigt. `socat` ist
optional, aber für die spätere Prüfung des Admin-Sockets praktisch.

```bash
# Ubuntu / Debian
sudo apt update
sudo apt install -y git curl ca-certificates build-essential pkg-config nano socat

# AlmaLinux
sudo dnf group install -y "Development Tools"
sudo dnf install -y git curl ca-certificates pkgconf-pkg-config nano socat
```

Die aktuellen Abhängigkeiten von Seda benötigen `pkg-config` nicht direkt; das
Paket ist für native Cargo-Abhängigkeiten und spätere Erweiterungen
mitinstalliert. Zur Laufzeit benötigt das fertige Seda-Binary diese
Build-Pakete nicht.

Für rustup, `git clone` und den ersten Cargo-Build benötigt der Host DNS sowie
ausgehendes HTTPS (TCP 443), insbesondere zu GitHub, `rustup.rs`,
`static.rust-lang.org`, `crates.io` und `static.crates.io`. Hinter einem
HTTP(S)-Proxy vor den folgenden Befehlen zum Beispiel setzen:

```bash
export HTTPS_PROXY=http://proxy.example:3128
export HTTP_PROXY=http://proxy.example:3128
```

Diese Variablen werden von Git, curl, rustup und Cargo verwendet. Für den
lokalen Seda-Port 9080 ist beim Herunterladen oder Bauen keine
Firewall-Freigabe erforderlich.

Rust **1.75 oder neuer** wird am zuverlässigsten über
[rustup](https://rustup.rs/) für den angemeldeten Build-Benutzer installiert:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
. "$HOME/.cargo/env"
rustup toolchain install stable
rustup default stable
rustc --version
cargo --version
```

Die von `rustc --version` gemeldete Version muss mindestens 1.75 sein.
Distributionspakete für Rust sind ebenfalls möglich, sofern sie diese
Mindestversion erfüllen.

## 2. Repository auf den Zielhost holen

Quellcode und Build-Artefakte liegen getrennt von der späteren
Laufzeitinstallation:

- `/usr/local/src/seda`: Git-Checkout und Build-Artefakte
- `/usr/local/bin/seda`: installiertes Programm
- `/etc/seda`: Konfiguration
- `/var/lib/seda`: persistenter Zustand und optionale Inhalte
- `/run/seda`: flüchtiger Admin-Socket, von systemd erzeugt

Das Quellverzeichnis anlegen und das Repository über HTTPS klonen:

```bash
sudo install -d -o "$USER" -g "$(id -gn)" /usr/local/src/seda
git clone https://github.com/WPreier/seda.git /usr/local/src/seda
cd /usr/local/src/seda
```

Zurzeit gibt es für dieses Projekt keine veröffentlichten GitHub-Releases und
damit auch kein offiziell bereitgestelltes Linux-Binary oder Release-Tarball.
Ein beliebiges Binary aus anderer Quelle sollte nicht verwendet werden. Sobald
signierte Release-Artefakte veröffentlicht werden, können sie den Build
ersetzen; Herkunft, CPU-Architektur und Prüfsumme müssen dann vor der
Installation geprüft werden.

## 3. Bauen und Binary installieren

Im geklonten Repository:

```bash
cargo test --locked
cargo build --locked --release
./target/release/seda --version
sudo install -o root -g root -m 0755 target/release/seda /usr/local/bin/seda
```

`Cargo.toml` ist die Quelle der Programmversion; `seda --version` muss dieselbe
`MAJOR.MINOR.PATCH`-Version ausgeben. Das Quellverzeichnis wird nach der
Installation noch benötigt, um die mitgelieferte systemd-Unit zu kopieren und
später Updates zu bauen.

## 4. Benutzer und Verzeichnisse

```bash
sudo groupadd --system seda 2>/dev/null || true
sudo useradd --system --gid seda --home-dir /var/lib/seda \
  --shell /usr/sbin/nologin seda 2>/dev/null || true

sudo install -d -o root -g seda -m 0750 /etc/seda
sudo install -d -o seda -g seda -m 0750 /var/lib/seda
sudo install -d -o root -g root -m 0750 /run/seda
```

`/etc/seda` enthält die Konfiguration, `/var/lib/seda` den aktiven Zustand und
`/run/seda` den flüchtigen Admin-Socket. systemd erzeugt `/run/seda` bei jedem
Start neu. Der Socket selbst wird von Seda bewusst mit Modus `0600` angelegt.

Die Standardkonfiguration wird mit der echten CLI erzeugt:

```bash
cd /var/lib/seda
sudo /usr/local/bin/seda --config /etc/seda/seda.toml --resetconfig
sudo chown root:seda /etc/seda/seda.toml
sudo chmod 0640 /etc/seda/seda.toml
```

Danach die Datei öffnen und mindestens diese Werte setzen:

```bash
sudo nano /etc/seda/seda.toml
```

```toml
[server]
listen = "127.0.0.1:9080"
state_dir = "/var/lib/seda"
admin_socket = "/run/seda/admin.sock"
user = "seda"
group = "seda"

[trust]
allowed_peers = ["127.0.0.1/32", "::1/128"]

[proxy]
protocol = "auto"
require_proxy = false
```

Optionale Inhalte aus dem Dateisystem können als Site eingebunden werden:

```toml
[[sites]]
path = "/site"
root = "/var/lib/seda/site"

[[sites]]
path = "/site/handbuch"
root = "/srv/seda/site/handbuch"
```

`path` ist das URL-Präfix, `root` das zugehörige Inhaltsverzeichnis. Bei
überlappenden Präfixen gewinnt das längste passende Präfix; im Beispiel wird
`/site/handbuch` daher aus `/srv/seda/site/handbuch` bedient. Seda liest die
Dateien einmal beim Start beziehungsweise Reload vollständig in den
Arbeitsspeicher ein; Änderungen auf der Platte werden erst beim nächsten
Reload sichtbar.

Falls `[[sites]]` lokale Inhalte lädt, benötigt der Startprozess Leserechte auf
diese Pfade. Geheimnisse oder private Schlüssel gehören nicht in Seda; falls
eine lokale Erweiterung dennoch Secret-Dateien verwendet, sollten sie
`root:seda`, Modus `0600` (oder höchstens `0640`, wenn Seda sie lesen muss)
haben. Konfigurations- und Site-Verzeichnisse dürfen für `seda` nicht
schreibbar sein. Der State-Pfad muss dagegen schreibbar bleiben:

```bash
sudo chown -R seda:seda /var/lib/seda
sudo chmod 0750 /var/lib/seda
sudo /usr/local/bin/seda -f /etc/seda/seda.toml --validate
```

`-f` und `--config` sind gleichwertig. `--resetconfig` überschreibt die
Konfiguration mit dem eingebetteten Default; deshalb nur bewusst und nach einem
Backup verwenden.

## 5. systemd

Die mitgelieferte Unit verwendet `/usr/local/bin/seda`,
`/etc/seda/seda.toml`, `/var/lib/seda` und `/run/seda`:

```bash
cd /usr/local/src/seda
sudo install -o root -g root -m 0644 deploy/seda.service \
  /etc/systemd/system/seda.service
sudo systemctl daemon-reload
sudo systemctl enable --now seda
sudo systemctl status seda
sudo journalctl -u seda -n 100 --no-pager
```

Die Unit startet als root, bindet Listener und Admin-Socket und Seda wechselt
danach gemäß `server.user`/`server.group` zum unprivilegierten Benutzer. Diese
beiden Config-Werte deshalb nicht entfernen. Bei abweichenden Installationspfaden
`ExecStart`, `WorkingDirectory`, `StateDirectory`, `ConfigurationDirectory` und
`RuntimeDirectory` gemeinsam anpassen.

Ein Reload ist fail-closed: Eine ungültige neue Config lässt den alten Zustand
aktiv.

```bash
sudo systemctl reload seda
# alternativ:
sudo kill -HUP "$(systemctl show -p MainPID --value seda)"
```

Der lokale Admin-Socket versteht `status`, `reload`, `help` und `quit`. Wegen
Modus `0600` wird er standardmäßig als root angesprochen, zum Beispiel mit
`sudo socat - UNIX-CONNECT:/run/seda/admin.sock`.

## 6. Netzwerk, Trust und Firewall

- Bevorzugt auf `127.0.0.1:9080` lauschen. Niemals `0.0.0.0` oder eine
  öffentliche Adresse verwenden; Seda lehnt Wildcard- und öffentliche
  Bind-Adressen zusätzlich selbst ab.
- Bei SeDat auf demselben Host bleibt `allowed_peers` auf Loopback. SeDat kann
  PROXY v2 senden; dann `protocol = "v2"` und `require_proxy = true` setzen.
- Apache und Nginx senden zum HTTP-Upstream üblicherweise **kein** PROXY
  Protocol. Für einen lokalen Upstream `protocol = "auto"` (oder `"off"`) und
  `require_proxy = false` verwenden. `X-Forwarded-For`, `Forwarded` und
  `X-Real-IP` sind keine Seda-Vertrauensquelle.
- Bei einem getrennten privaten Backend-Netz an eine private Adresse binden und
  in `allowed_peers` möglichst nur die exakten Proxy-Adressen (`/32` bzw.
  `/128`) eintragen. Nur diese Proxy-Hosts dürfen Port 9080 in UFW, nftables
  oder firewalld erreichen. Keine öffentliche Portfreigabe anlegen.

Beispiel für AlmaLinux/firewalld bei einem getrennten Proxy
`10.20.0.10` (nur erforderlich, wenn Seda nicht auf Loopback lauscht):

```bash
sudo firewall-cmd --permanent --add-rich-rule='rule family=ipv4 source address=10.20.0.10/32 port port=9080 protocol=tcp accept'
sudo firewall-cmd --reload
```

## 7. Prüfung

Mit der Standardkonfiguration ohne erzwungenes PROXY Protocol:

```bash
curl --fail --show-error http://127.0.0.1:9080/seda
curl --fail --show-error http://127.0.0.1:9080/sedat
sudo systemctl reload seda
sudo journalctl -u seda -n 50 --no-pager
```

`/seda` und `/sedat` werden immer bereitgestellt, solange die eingebettete
Dokumentation in der Konfiguration aktiviert ist:

```toml
[embedded_docs]
enabled = true
sedat_path = "/sedat"
seda_path = "/seda"
```

Mit `enabled = false` werden diese beiden Pfade nicht bereitgestellt.

Bei `require_proxy = true` schlägt ein direktes `curl` erwartungsgemäß fehl;
dann über SeDat testen oder einen korrekten PROXY-v2-Testclient verwenden.

## 8. Upgrade und Rollback

Version 0.1.2 erhöht den Patchstand gegenüber 0.1.1. Aktualisiert wurden der
Bare-Host-Bootstrap, ein `[[sites]]`-Beispiel, der Prüfhinweis für
`embedded_docs` und das explizite `cd` vor der Installation der systemd-Unit.

Vor jedem Update Config, Unit und das bisherige Binary sichern. Releases folgen
SemVer; bei jedem Update/Release wird mindestens `PATCH` erhöht. Neues Binary
zuerst mit `--version` und die bestehende Config mit `--validate` prüfen, dann
atomar unter `/usr/local/bin/seda` austauschen und `systemctl restart seda`
ausführen. Für ein Rollback das gesicherte Binary und gegebenenfalls die
passende Config wiederherstellen und erneut validieren. Ein Binary kann nicht
im laufenden Prozess per SIGHUP ersetzt werden.

## 9. SELinux auf AlmaLinux

SELinux sollte aktiviert und enforcing bleiben. Standardpfade unter
`/usr/local/bin`, `/etc`, `/var/lib` und `/run` funktionieren häufig ohne
Sonderregel. Bei einer AVC-Verweigerung zuerst Diagnose und Labels prüfen:

```bash
getenforce
ls -lZ /usr/local/bin/seda /etc/seda /var/lib/seda
sudo restorecon -Rv /usr/local/bin/seda /etc/seda /var/lib/seda
sudo ausearch -m AVC -ts recent
```

Für abweichende Pfade dauerhafte Kontexte mit `semanage fcontext` plus
`restorecon` definieren (Paket `policycoreutils-python-utils`). Permissive Mode
kann kurzfristig zur Diagnose dienen, ist aber keine dauerhafte Lösung; SELinux
nicht pauschal deaktivieren. Aus AVC-Meldungen nur eine möglichst enge lokale
Policy ableiten.
