#!/bin/sh
#
# Serves the demo's web bundle with Caddy on http://127.0.0.1:8070, or on the
# address given.
#
#   ./scripts/buck run //crates/beui-web-demo:web-serve [-- HOST:PORT]
set -eu
caddy="$1" caddyfile="$2" bundle="$3"
listen="${4:-127.0.0.1:8070}"
[ -x "$caddy/caddy" ] && caddy="$caddy/caddy" || caddy="$caddy/caddy.exe"
echo "Serving http://$listen"
BEUI_DEMO_ADDRESS="http://$listen" BEUI_DEMO_ROOT="$(cd "$bundle" && pwd)" \
    "$caddy" run --adapter caddyfile --config "$caddyfile"
