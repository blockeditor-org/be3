#!/bin/bash
# Run as root on the scheduler VM (created in the nl-scheduler environment). Files are in /tmp/nl.
set -euo pipefail; cd /tmp/nl; export DEBIAN_FRONTEND=noninteractive
apt-get install -y -qq wireguard-tools zstd >/dev/null 2>&1 || { apt-get update -qq && apt-get install -y -qq wireguard-tools zstd >/dev/null; }
id nativelink >/dev/null 2>&1 || useradd -r -m -d /var/lib/nativelink -s /usr/sbin/nologin nativelink
# WireGuard hub
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
install -d /etc/systemd/system/wg-quick@wg-sch.service.d; install -m0644 wg-quick-retry.conf /etc/systemd/system/wg-quick@wg-sch.service.d/retry.conf; systemctl daemon-reload
systemctl enable --now wg-quick@wg-sch
install -m0755 nativelink /usr/local/bin/nativelink; install -m0755 nl-small-pack.sh nl-small-restore.sh add-peer.sh remove-peer.sh /usr/local/bin/
# NativeLink + the Bunny full-access key (delivered by the environment as ~/nl-bunny.env)
install -d -m0750 -g nativelink /etc/nativelink
install -m0640 -g nativelink /home/user/nl-bunny.env /etc/nativelink/server.env; chmod 600 /home/user/nl-bunny.env
install -m0640 -g nativelink server.json5 /etc/nativelink/server.json5
install -m0644 nativelink-server.service nl-small-pack.service nl-small-pack.timer /etc/systemd/system/
systemctl daemon-reload; systemctl enable --now nativelink-server   # the pack timer is enabled at cutover
sleep 6; systemctl is-active nativelink-server; ss -tlnp | grep -c -E "10.100.0.1:5(2|0)"
