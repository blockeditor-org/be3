#!/bin/sh
#
# Serves the web bundle: starts be-server on --backend and Caddy with the
# Caddyfile beside this. Without --domain, Caddy serves plain http on --addr,
# listening only on its host: loopback unless given another, such as
# `--addr 0.0.0.0:8080` to reach it from another machine (guides/hosting.md,
# "Listening"). With --domain, Caddy serves that domain over https on ports 80
# and 443 of every interface, with a certificate it gets for it, and --addr is
# unused. Other arguments go to be-server, which refuses sign-ins and new
# accounts unless given --allow-login and --allow-registration.
#
#   ./scripts/buck run //crates/be-host:web [-- --addr HOST:PORT] [--backend HOST:PORT]
#   ./scripts/buck run //crates/be-host:web -- --domain api.be3.pfg.pw [--data-dir DIR]
set -eu
caddy="$1" caddyfile="$2" server="$3" bundle="$4"
shift 4
address=127.0.0.1:8080
backend=127.0.0.1:9090
domain=
count=$#
index=0
while [ "$index" -lt "$count" ]; do
    argument="$1"
    shift
    index=$((index + 1))
    case "$argument" in
        --addr) address="$1"; shift; index=$((index + 1)) ;;
        --backend) backend="$1"; shift; index=$((index + 1)) ;;
        --domain) domain="$1"; shift; index=$((index + 1)) ;;
        *) set -- "$@" "$argument" ;;
    esac
done

case "$address" in
    *:*) ;;
    *) echo "--addr takes HOST:PORT, such as 127.0.0.1:8080, not $address." >&2; exit 1 ;;
esac
[ -x "$caddy/caddy" ] && caddy="$caddy/caddy" || caddy="$caddy/caddy.exe"
if [ -n "$domain" ]; then
    site="$domain"
    bind="[::]"
    origin="https://$domain"
else
    # Any Host when the address is a wildcard, else only the names it is
    # served as, so a page that rebinds its own name to loopback is not
    # answered (guides/hosting.md, "Listening").
    port="${address##*:}"
    bind="${address%:*}"
    case "$bind" in
        0.0.0.0 | "[::]") site="http://:$port" ;;
        127.0.0.1 | localhost) site="http://127.0.0.1:$port, http://localhost:$port" ;;
        *) site="http://$address" ;;
    esac
    origin="http://$address"
fi
echo "Serving $origin"
"$server" --build-info-file "$bundle/build-info.json" --addr "$backend" --auth-origin "$origin" "$@" &
server_pid=$!
trap 'kill "$server_pid" 2> /dev/null || true' EXIT INT TERM
BE3_SITE="$site" BE3_BIND="$bind" BE3_BACKEND_URL="$backend" BE3_WEB_ROOT="$(cd "$bundle" && pwd)" \
    "$caddy" run --adapter caddyfile --config "$caddyfile"
