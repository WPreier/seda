# Seda unter Linux installieren

Diese Anleitung gilt für Ubuntu, Debian und AlmaLinux. Seda ist ein internes
HTTP-Backend: Es darf nicht direkt aus dem Internet erreichbar sein. SeDat,
Apache oder Nginx übernimmt die öffentliche Edge-/TLS-Funktion.

## 1. Voraussetzungen und Binary

Für ein vorhandenes, zur Distribution und CPU passendes Release-Binary sind nur
`curl` und systemd erforderlich. Vor der Installation immer Herkunft und
Prüfsumme des Binaries prüfen.

Zum Bauen aus dem Quellcode wird Rust **1.75 oder neuer** benötigt:

```bash
# Ubuntu / Debian
sudo apt update
sudo apt install -y build-essential curl

# AlmaLinux
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y curl
```

Rust kann distributionsweit paketiert oder für einen Build-Benutzer über
[rustup](https://rustup.rs/) installiert werden. Im Repository:

```bash
cargo test --locked
cargo build --locked --release
./target/release/seda --version
sudo install -o root -g root -m 0755 target/release/seda /usr/local/bin/seda
```

Bei einem vorgebauten Binary ersetzt dessen Pfad
`target/release/seda`. `Cargo.toml` ist die Quelle der Programmversion;
`seda --version` muss dieselbe `MAJOR.MINOR.PATCH`-Version ausgeben.

## 2. Benutzer und Verzeichnisse

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

Danach mindestens diese Werte in `/etc/seda/seda.toml` setzen:

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

## 3. systemd

Die mitgelieferte Unit verwendet `/usr/local/bin/seda`,
`/etc/seda/seda.toml`, `/var/lib/seda` und `/run/seda`:

```bash
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

## 4. Netzwerk, Trust und Firewall

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

## 5. Prüfung

Mit der Standardkonfiguration ohne erzwungenes PROXY Protocol:

```bash
curl --fail --show-error http://127.0.0.1:9080/seda
curl --fail --show-error http://127.0.0.1:9080/sedat
sudo systemctl reload seda
sudo journalctl -u seda -n 50 --no-pager
```

Bei `require_proxy = true` schlägt ein direktes `curl` erwartungsgemäß fehl;
dann über SeDat testen oder einen korrekten PROXY-v2-Testclient verwenden.

## 6. Upgrade und Rollback

Vor jedem Update Config, Unit und das bisherige Binary sichern. Releases folgen
SemVer; bei jedem Update/Release wird mindestens `PATCH` erhöht. Neues Binary
zuerst mit `--version` und die bestehende Config mit `--validate` prüfen, dann
atomar unter `/usr/local/bin/seda` austauschen und `systemctl restart seda`
ausführen. Für ein Rollback das gesicherte Binary und gegebenenfalls die
passende Config wiederherstellen und erneut validieren. Ein Binary kann nicht
im laufenden Prozess per SIGHUP ersetzt werden.

## 7. SELinux auf AlmaLinux

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
