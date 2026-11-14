#!/bin/bash
# Run as root on a new Hetzner server (Ubuntu 24.04): `scp -r` scripts/internal/nativelink and scripts/internal/re-relay there, side by side, and run it with the server's public
# name, `bash setup.sh nl.be3.pfg.pw` (its DNS record must point at the server). It installs NativeLink (cache, scheduler and worker in one process,
# nativelink.json5), Caddy (Caddyfile) and the dashboard (dashboard/), and makes Caddy's tokens on first run. Running it again
# updates them all.
set -euo pipefail; cd "$(dirname "$(readlink -f "$0")")"; export DEBIAN_FRONTEND=noninteractive
NL_HOST=${1:?usage: setup.sh PUBLIC_NAME}
NL_VERSION=1.8.3
NL_SHA256=1a6596c96f059d4e99f7352164321e1f1cdff435275667ad8489e83f7e2977c5   # GitHub's published digest
CADDY_VERSION=2.11.7
CADDY_SHA256=727b91701a392de6ebc5027509f548bf39979e5216340d0faed8fa5e69c84f8b   # Caddy's published digest
IMAGE=docker.io/library/buildpack-deps@sha256:2607512c685336a441eba9719ab17da07137ab3178ae8b7118dfe1dff7991549   # buck/tools/defs.bzl worker_properties()
# Only key logins (Hetzner's image allows passwords for users other than root).
printf 'PasswordAuthentication no\nKbdInteractiveAuthentication no\n' > /etc/ssh/sshd_config.d/10-keys-only.conf
systemctl reload ssh
# 16 GB of swap, which the server's image lacks: the worker does not schedule by memory, so a release build's LTO
# link (about 10 GB) beside 15 other actions can run out, and swap slows that down rather than OOM-killing the link.
# Swappiness 10 keeps the CAS's files in the page cache rather than swapping out what runs.
if [ ! -e /swapfile ]; then fallocate -l 16G /swapfile; chmod 600 /swapfile; mkswap /swapfile >/dev/null; fi
swapon --show=NAME --noheadings | grep -qx /swapfile || swapon /swapfile
grep -q '^/swapfile ' /etc/fstab || echo '/swapfile none swap sw 0 0' >> /etc/fstab
echo 'vm.swappiness = 10' > /etc/sysctl.d/90-swap.conf; sysctl -q -p /etc/sysctl.d/90-swap.conf
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
# ../re-relay with a pinned Go.
GO_VERSION=1.26.11
GO_SHA256=9849f4e04ee35303714e34fbc3ac8ab44ea3e448a95aa182f91e3b9b79c710ec   # go.dev's published digest
RELAY_SOURCE=$(readlink -f ../re-relay)
if [ "$(/usr/local/go/bin/go version 2>/dev/null)" != "go version go$GO_VERSION linux/amd64" ]; then
  curl -fsSL --retry 3 -o /tmp/go.tgz "https://go.dev/dl/go$GO_VERSION.linux-amd64.tar.gz"
  echo "$GO_SHA256  /tmp/go.tgz" | sha256sum -c -
  rm -rf /usr/local/go; tar xzf /tmp/go.tgz -C /usr/local
fi
( cd "$RELAY_SOURCE" && CGO_ENABLED=0 GOTOOLCHAIN=local /usr/local/go/bin/go build -trimpath -o /usr/local/bin/re-relay . )
install -d /etc/nativelink /etc/caddy /var/lib/nativelink
install -m0644 nativelink.json5 /etc/nativelink/nativelink.json5
install -m0644 Caddyfile dashboard/gate.html /etc/caddy/
# Caddy's tokens, each made once: clients' BE3_NL_TOKEN and the dashboard's cookie (guides/build_server.md).
token() { head -c 32 /dev/urandom | base64 | tr -dc 'A-Za-z0-9'; }
( umask 077; touch /etc/nl-caddy.env
  grep -q '^NL_TUNNEL_TOKEN=.' /etc/nl-caddy.env || echo "NL_TUNNEL_TOKEN=$(token)" >> /etc/nl-caddy.env
  grep -q '^NL_DASH_TOKEN=.' /etc/nl-caddy.env || echo "NL_DASH_TOKEN=$(token)" >> /etc/nl-caddy.env )
sed -i '/^NL_HOST=/d' /etc/nl-caddy.env; echo "NL_HOST=$NL_HOST" >> /etc/nl-caddy.env

# The dashboard (dashboard/, guides/build_server.md): node_exporter for the machine's metrics, the OpenTelemetry
# Collector for NativeLink's, Prometheus to keep them, nl-stats.py for the builds' records, and Grafana to show them.
PROMETHEUS_IMAGE=docker.io/prom/prometheus@sha256:fa3302279710be2f21a8301ec17a8b4f2301fd829bd113839b3b6b02b6f191a2   # v3.15.1
OTEL_IMAGE=docker.io/otel/opentelemetry-collector-contrib@sha256:39923a8e431bd1f57be82411999d389fcfe40857492e4365456d97a4c1f74be6   # 0.162.0
GRAFANA_IMAGE=docker.io/grafana/grafana@sha256:9b031995a5483a44f80a5dd214a70cecb31448b42d765b44d3756e17b855775f   # 13.0.13
SQLITE_PLUGIN_VERSION=4.0.6
SQLITE_PLUGIN_SHA256=e5ce2354dd26ba9bea56b37f9ed4050cc917a46b858a0a191e1cafda382bf18a   # grafana.com's published digest
dpkg -s prometheus-node-exporter sqlite3 >/dev/null 2>&1 || { apt-get update -qq && apt-get install -y -qq --no-install-recommends prometheus-node-exporter sqlite3 >/dev/null; }
echo 'ARGS="--web.listen-address=127.0.0.1:9100"' > /etc/default/prometheus-node-exporter
for image in "$PROMETHEUS_IMAGE" "$OTEL_IMAGE" "$GRAFANA_IMAGE"; do docker pull -q "$image" >/dev/null; done
install -d /etc/nl-dashboard/grafana/provisioning/datasources /etc/nl-dashboard/grafana/provisioning/dashboards \
  /etc/nl-dashboard/grafana/provisioning/plugins /etc/nl-dashboard/grafana/provisioning/alerting \
  /etc/nl-dashboard/grafana/dashboards /usr/local/lib/nl-stats /var/lib/nl-stats /var/lib/prometheus/node-exporter
install -d -o 65534 -g 65534 /var/lib/nl-prometheus
install -d -o 472 -g 0 /var/lib/nl-grafana /var/lib/nl-grafana/plugins
install -m0644 dashboard/prometheus.yml dashboard/otel-collector.yaml /etc/nl-dashboard/
install -m0644 dashboard/grafana-datasources.yaml /etc/nl-dashboard/grafana/provisioning/datasources/
install -m0644 dashboard/grafana-dashboards.yaml /etc/nl-dashboard/grafana/provisioning/dashboards/
install -m0644 dashboard/build-server.json /etc/nl-dashboard/grafana/dashboards/
install -m0644 dashboard/nl-stats.py /usr/local/lib/nl-stats/
if [ "$(cat /var/lib/nl-grafana/plugins/frser-sqlite-datasource.version 2>/dev/null)" != "$SQLITE_PLUGIN_VERSION" ]; then
  curl -fsSL --retry 3 -o /tmp/sqlite-plugin.zip \
    "https://grafana.com/api/plugins/frser-sqlite-datasource/versions/$SQLITE_PLUGIN_VERSION/download"
  echo "$SQLITE_PLUGIN_SHA256  /tmp/sqlite-plugin.zip" | sha256sum -c -
  rm -rf /var/lib/nl-grafana/plugins/frser-sqlite-datasource
  python3 -m zipfile -e /tmp/sqlite-plugin.zip /var/lib/nl-grafana/plugins/
  chmod 0755 /var/lib/nl-grafana/plugins/frser-sqlite-datasource/gpx_*   # zipfile keeps no modes
  echo "$SQLITE_PLUGIN_VERSION" > /var/lib/nl-grafana/plugins/frser-sqlite-datasource.version
  chown -R 472:0 /var/lib/nl-grafana/plugins
fi

UNITS="nativelink nl-caddy nl-tunnel nl-stats nl-prometheus nl-otel nl-grafana prometheus-node-exporter"
install -m0644 nativelink.service nl-caddy.service nl-tunnel.service dashboard/nl-stats.service \
  dashboard/nl-prometheus.service dashboard/nl-otel.service dashboard/nl-grafana.service /etc/systemd/system/
systemctl daemon-reload
systemctl enable $UNITS
systemctl restart $UNITS
sleep 5; systemctl is-active $UNITS
