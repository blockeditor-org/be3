#!/bin/bash
# Run as root on a new Hetzner server (Ubuntu 24.04): `scp -r` this directory and scripts/internal/re-relay there, side by side, and run it with the server's public
# name, `bash setup.sh nl.be3.pfg.pw` (its DNS record must point at the server). It installs NativeLink (cache, scheduler and worker in one process,
# nativelink.json5) and Caddy (Caddyfile), and makes Caddy's token on first run. Running it again updates both.
set -euo pipefail; cd "$(dirname "$(readlink -f "$0")")"; export DEBIAN_FRONTEND=noninteractive
NL_HOST=${1:?usage: setup.sh PUBLIC_NAME}
NL_VERSION=1.8.3
NL_SHA256=1a6596c96f059d4e99f7352164321e1f1cdff435275667ad8489e83f7e2977c5   # GitHub's published digest; as in ../scheduler/setup-scheduler.sh
CADDY_VERSION=2.11.7
CADDY_SHA256=727b91701a392de6ebc5027509f548bf39979e5216340d0faed8fa5e69c84f8b   # as in ../scheduler/setup-scheduler.sh
IMAGE=docker.io/library/buildpack-deps@sha256:2607512c685336a441eba9719ab17da07137ab3178ae8b7118dfe1dff7991549   # buck/tools/defs.bzl worker_properties()
# Only key logins (Hetzner's image allows passwords for users other than root).
printf 'PasswordAuthentication no\nKbdInteractiveAuthentication no\n' > /etc/ssh/sshd_config.d/10-keys-only.conf
systemctl reload ssh
command -v docker >/dev/null || { apt-get update -qq && apt-get install -y -qq docker.io >/dev/null; }
systemctl enable --now docker
docker pull -q "$IMAGE" >/dev/null
curl -fsSL --retry 3 -o /tmp/nl.tgz "https://github.com/TraceMachina/nativelink/releases/download/v$NL_VERSION/nativelink-$NL_VERSION-x86_64-unknown-linux-musl.tar.gz"
echo "$NL_SHA256  /tmp/nl.tgz" | sha256sum -c -
tar xzf /tmp/nl.tgz -C /tmp nativelink; install -m0755 /tmp/nativelink /usr/local/bin/nativelink
curl -fsSL --retry 3 -o /tmp/caddy.tgz "https://github.com/caddyserver/caddy/releases/download/v$CADDY_VERSION/caddy_${CADDY_VERSION}_linux_amd64.tar.gz"
echo "$CADDY_SHA256  /tmp/caddy.tgz" | sha256sum -c -
tar xzf /tmp/caddy.tgz -C /tmp caddy; install -m0755 /tmp/caddy /usr/local/bin/caddy
# re-relay, whose tunnel-server carries the calls of a client behind a slow proxy (nl-tunnel.service), built from
# ../../re-relay, or ../re-relay when the two directories were copied side by side, with a pinned Go.
GO_VERSION=1.26.11
GO_SHA256=9849f4e04ee35303714e34fbc3ac8ab44ea3e448a95aa182f91e3b9b79c710ec   # go.dev's published digest
RELAY_SOURCE=$(readlink -f ../../re-relay 2>/dev/null || true); [ -f "$RELAY_SOURCE/go.mod" ] || RELAY_SOURCE=$(readlink -f ../re-relay)
if [ "$(/usr/local/go/bin/go version 2>/dev/null)" != "go version go$GO_VERSION linux/amd64" ]; then
  curl -fsSL --retry 3 -o /tmp/go.tgz "https://go.dev/dl/go$GO_VERSION.linux-amd64.tar.gz"
  echo "$GO_SHA256  /tmp/go.tgz" | sha256sum -c -
  rm -rf /usr/local/go; tar xzf /tmp/go.tgz -C /usr/local
fi
( cd "$RELAY_SOURCE" && CGO_ENABLED=0 GOTOOLCHAIN=local /usr/local/go/bin/go build -trimpath -o /usr/local/bin/re-relay . )
install -d /etc/nativelink /etc/caddy /var/lib/nativelink
install -m0644 nativelink.json5 /etc/nativelink/nativelink.json5
install -m0644 Caddyfile /etc/caddy/Caddyfile
# Caddy's token, made once: clients' BE3_NL_TOKEN (guides/build_server.md).
if [ ! -s /etc/nl-caddy.env ]; then
  ( umask 077; echo "NL_TUNNEL_TOKEN=$(head -c 32 /dev/urandom | base64 | tr -dc 'A-Za-z0-9')" > /etc/nl-caddy.env )
fi
sed -i '/^NL_HOST=/d' /etc/nl-caddy.env; echo "NL_HOST=$NL_HOST" >> /etc/nl-caddy.env
install -m0644 nativelink.service nl-caddy.service nl-tunnel.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable nativelink nl-caddy nl-tunnel
systemctl restart nativelink nl-caddy nl-tunnel
sleep 5; systemctl is-active nativelink nl-caddy nl-tunnel
