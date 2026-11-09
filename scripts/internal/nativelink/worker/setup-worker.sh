#!/bin/bash
# Run as root on a worker VM. Expects in /tmp/nl: wg-nl.conf nativelink worker.json5 nativelink-worker.service
set -euo pipefail
cd /tmp/nl
export DEBIAN_FRONTEND=noninteractive
apt-get install -y -qq wireguard-tools >/dev/null 2>&1 || { apt-get update -qq && apt-get install -y -qq wireguard-tools >/dev/null; }
install -d -m 700 /etc/wireguard; install -m 600 wg-nl.conf /etc/wireguard/wg-nl.conf
ufw allow 51820/udp >/dev/null 2>&1 || true
systemctl enable --now wg-quick@wg-nl
install -m 0755 nativelink /usr/local/bin/nativelink
install -d /etc/nativelink /var/lib/nativelink-worker
if [ -f /home/user/nl-worker.env ]; then install -m 0600 /home/user/nl-worker.env /etc/nativelink/worker.env && rm -f /home/user/nl-worker.env; else install -m 0600 /dev/null /etc/nativelink/worker.env; fi
install -m 0644 worker.json5 /etc/nativelink/worker.json5
install -m 0644 nativelink-worker.service /etc/systemd/system/nativelink-worker.service
docker pull -q docker.io/library/buildpack-deps@sha256:2607512c685336a441eba9719ab17da07137ab3178ae8b7118dfe1dff7991549 >/dev/null
systemctl daemon-reload; systemctl enable --now nativelink-worker
sleep 3; ping -c1 -W3 10.99.0.1 >/dev/null && echo "wg ok"; systemctl is-active nativelink-worker
