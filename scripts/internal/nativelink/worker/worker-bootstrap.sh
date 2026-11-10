#!/bin/bash
# Runs once on a new worker VM (`boat new --setup-file`), with no SSH. The controller on the scheduler hands the VM,
# at creation (`--file`), everything that differs per worker:
#   ~/nl-wg.conf          the WireGuard spoke config (its own key, the scheduler as the hub)
#   ~/nl-worker.json5     the worker config (hub address filled in)
#   ~/nl-worker.service   the systemd unit
#   ~/worker-rw.env       the Bunny key it writes objects of 16 KB and up with (the zone's full-access one)
set -euo pipefail
exec > >(tee -a /tmp/nl-bootstrap.log) 2>&1
NL_VERSION=1.8.3
NL_SHA256=1a6596c96f059d4e99f7352164321e1f1cdff435275667ad8489e83f7e2977c5   # GitHub's published digest of the release tarball
NL_URL=https://github.com/TraceMachina/nativelink/releases/download/v$NL_VERSION/nativelink-$NL_VERSION-x86_64-unknown-linux-musl.tar.gz
IMAGE=docker.io/library/buildpack-deps@sha256:2607512c685336a441eba9719ab17da07137ab3178ae8b7118dfe1dff7991549   # buck/tools/defs.bzl worker_properties()
cd "$HOME"
export DEBIAN_FRONTEND=noninteractive
( sudo apt-get install -y -qq wireguard-tools >/dev/null 2>&1 || { sudo apt-get update -qq && sudo apt-get install -y -qq wireguard-tools >/dev/null; } ) &
apt_pid=$!   # wait for this job only: a bare `wait` would also wait for the logging tee below and never return
curl -fsSL --retry 3 -o /tmp/nl.tgz "$NL_URL"
echo "$NL_SHA256  /tmp/nl.tgz" | sha256sum -c -
tar xzf /tmp/nl.tgz -C /tmp nativelink
wait $apt_pid
sudo install -m0755 /tmp/nativelink /usr/local/bin/nativelink
sudo install -d -m700 /etc/wireguard; sudo install -m600 nl-wg.conf /etc/wireguard/wg-sch.conf; shred -u nl-wg.conf 2>/dev/null || rm -f nl-wg.conf
sudo install -d /etc/nativelink /var/lib/nativelink-worker
sudo install -m0644 nl-worker.json5 /etc/nativelink/worker.json5
grep -q '^BUNNY_S3_SECRET=.' worker-rw.env || { echo "no Bunny key in ~/worker-rw.env" >&2; exit 1; }
sudo install -m0600 worker-rw.env /etc/nativelink/worker.env; shred -u worker-rw.env 2>/dev/null || rm -f worker-rw.env
sudo install -m0644 nl-worker.service /etc/systemd/system/nativelink-worker.service
sudo ufw allow 51820/udp >/dev/null 2>&1 || true
sudo docker pull -q "$IMAGE" >/dev/null
sudo systemctl daemon-reload
sudo systemctl enable --now wg-quick@wg-sch
sudo systemctl enable --now nativelink-worker
sleep 3; ping -c1 -W3 10.100.0.1 >/dev/null && echo "wg ok" || echo "wg unreachable"
systemctl is-active nativelink-worker
