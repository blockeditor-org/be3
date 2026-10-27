#!/bin/sh
#
# Serves a web bundle with Caddy (bundle.Caddyfile) on DEFAULT, or on the
# address given after `--`: what `./scripts/buck run` of beui-web-demo's and
# beui-html-spike's :web runs.
#
#   serve-bundle.sh CADDY CADDYFILE DEFAULT BUNDLE [HOST:PORT]
set -eu
caddy="$1" caddyfile="$2" bundle="$4"
listen="${5:-$3}"
[ -x "$caddy/caddy" ] && caddy="$caddy/caddy" || caddy="$caddy/caddy.exe"
echo "Serving http://$listen"
BUNDLE_ADDRESS="http://$listen" BUNDLE_ROOT="$(cd "$bundle" && pwd)" \
    "$caddy" run --adapter caddyfile --config "$caddyfile"
