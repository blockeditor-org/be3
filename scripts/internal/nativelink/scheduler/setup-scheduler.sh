#!/bin/bash
# Run as root on a new scheduler VM (made in the nl-scheduler environment, so its secret files ~/nl-bunny.env and
# ~/nl-tunnel.env are there). Everything it installs is in this directory and ../worker; new-scheduler.sh copies both to
# the VM and runs this.
set -euo pipefail; cd "$(dirname "$(readlink -f "$0")")"; export DEBIAN_FRONTEND=noninteractive
NL_VERSION=1.8.3
NL_SHA256=1a6596c96f059d4e99f7352164321e1f1cdff435275667ad8489e83f7e2977c5   # GitHub's published digest of the tarball; worker-bootstrap.sh pins the same
NL_URL=https://github.com/TraceMachina/nativelink/releases/download/v$NL_VERSION/nativelink-$NL_VERSION-x86_64-unknown-linux-musl.tar.gz
apt-get install -y -qq wireguard-tools zstd >/dev/null 2>&1 || { apt-get update -qq && apt-get install -y -qq wireguard-tools zstd >/dev/null; }
id nativelink >/dev/null 2>&1 || useradd -r -m -d /var/lib/nativelink -s /usr/sbin/nologin nativelink
curl -fsSL --retry 3 -o /tmp/nl.tgz "$NL_URL"
echo "$NL_SHA256  /tmp/nl.tgz" | sha256sum -c -
tar xzf /tmp/nl.tgz -C /tmp nativelink
install -m0755 /tmp/nativelink /usr/local/bin/nativelink
# The OpenTelemetry collector that turns NativeLink's metrics into something the controller can read
OC_VERSION=0.165.0
OC_SHA256=bd6e73d94bf3359ad6fac25529a8ee308adb9b0b8a0e8af034ed3891fbe5aa33   # GitHub's published digest of the tarball
curl -fsSL --retry 3 -o /tmp/otelcol.tgz "https://github.com/open-telemetry/opentelemetry-collector-releases/releases/download/v$OC_VERSION/otelcol-contrib_${OC_VERSION}_linux_amd64.tar.gz"
echo "$OC_SHA256  /tmp/otelcol.tgz" | sha256sum -c -
tar xzf /tmp/otelcol.tgz -C /tmp otelcol-contrib
install -m0755 /tmp/otelcol-contrib /usr/local/bin/otelcol-contrib
install -d /etc/otelcol; install -m0644 otelcol.yaml /etc/otelcol/config.yaml
# Caddy, the front door boat's hosted port serves: it checks the token and forwards calls to NativeLink (Caddyfile)
CADDY_VERSION=2.11.7
CADDY_SHA256=727b91701a392de6ebc5027509f548bf39979e5216340d0faed8fa5e69c84f8b   # checked against the release's checksums file
curl -fsSL --retry 3 -o /tmp/caddy.tgz "https://github.com/caddyserver/caddy/releases/download/v$CADDY_VERSION/caddy_${CADDY_VERSION}_linux_amd64.tar.gz"
echo "$CADDY_SHA256  /tmp/caddy.tgz" | sha256sum -c -
tar xzf /tmp/caddy.tgz -C /tmp caddy
install -m0755 /tmp/caddy /usr/local/bin/caddy
install -d /etc/caddy; install -m0644 Caddyfile /etc/caddy/Caddyfile
# The WireGuard hub the workers join (clients do not: they come through Caddy)
install -d -m700 /etc/wireguard
[ -f /etc/wireguard/wg-sch.key ] || { umask 077; wg genkey > /etc/wireguard/wg-sch.key; }
wg pubkey < /etc/wireguard/wg-sch.key > /etc/wireguard/wg-sch.pub
( umask 077; cat > /etc/wireguard/wg-sch.conf <<CONF
[Interface]
Address = 10.100.0.1/24
ListenPort = 51822
PrivateKey = $(cat /etc/wireguard/wg-sch.key)
PostUp = ufw allow 51822/udp >/dev/null 2>&1 || true; ufw allow in on wg-sch >/dev/null 2>&1 || true
CONF
)
install -d /etc/systemd/system/wg-quick@wg-sch.service.d
install -m0644 wg-quick-retry.conf /etc/systemd/system/wg-quick@wg-sch.service.d/retry.conf
systemctl daemon-reload; systemctl enable --now wg-quick@wg-sch
install -m0755 nl-small-pack.sh nl-small-restore.sh add-peer.sh remove-peer.sh /usr/local/bin/
# NativeLink and the Bunny full-access key (the environment delivers it as ~/nl-bunny.env)
install -d -m0750 -g nativelink /etc/nativelink
install -m0640 -g nativelink /home/user/nl-bunny.env /etc/nativelink/server.env; chmod 600 /home/user/nl-bunny.env
# The token clients must send to Caddy, which nl-caddy.service reads from ~/nl-tunnel.env
[ -s /home/user/nl-tunnel.env ] || { echo "no ~/nl-tunnel.env: set it with boat env set-file nl-scheduler nl-tunnel.env" >&2; exit 1; }
chmod 600 /home/user/nl-tunnel.env
install -m0640 -g nativelink server.json5 /etc/nativelink/server.json5
install -m0644 nativelink-server.service nl-small-pack.service nl-small-pack.timer nl-caddy.service nl-otelcol.service /etc/systemd/system/
systemctl daemon-reload; systemctl enable --now nl-otelcol nativelink-server nl-caddy nl-small-pack.timer
sleep 6; systemctl is-active nativelink-server nl-caddy
# By hand, by someone allowed to expose a port: `host 8099 --localhost --public`, then `host url 8099 --localhost --public` prints the address
# that clients keep, with ?_token= and the token, as BE3_NL_URL (guides/build_server.md, "Starting over").
