#!/bin/sh
#
# Serves the web bundle: starts be-server on --backend and Caddy with the
# Caddyfile beside this. Without --domain, Caddy serves plain http on --listen
# on this machine. With --domain, Caddy serves that domain over https on ports
# 80 and 443, with a certificate it gets for it, and --listen is unused. Other
# arguments go to be-server; the default is --disable-registration.
#
#   ./scripts/buck run //crates/block-app:web-serve [-- --listen HOST:PORT] [--backend HOST:PORT]
#   ./scripts/buck run //crates/block-app:web-serve -- --domain blocks.pfg.pw [--data-dir DIR]
set -eu
caddy="$1" caddyfile="$2" bundle="$3" server="$4"
shift 4
listen=127.0.0.1:8080
backend=127.0.0.1:9090
domain=
count=$#
index=0
while [ "$index" -lt "$count" ]; do
    argument="$1"
    shift
    index=$((index + 1))
    case "$argument" in
        --listen) listen="$1"; shift; index=$((index + 1)) ;;
        --backend) backend="$1"; shift; index=$((index + 1)) ;;
        --domain) domain="$1"; shift; index=$((index + 1)) ;;
        *) set -- "$@" "$argument" ;;
    esac
done
[ $# -gt 0 ] || set -- --disable-registration

[ -x "$caddy/caddy" ] && caddy="$caddy/caddy" || caddy="$caddy/caddy.exe"
"$server" --addr "$backend" "$@" &
server_pid=$!
trap 'kill "$server_pid" 2> /dev/null || true' EXIT INT TERM
if [ -n "$domain" ]; then
    address="$domain"
    echo "Serving https://$domain"
else
    address="http://$listen"
    echo "Serving $address"
fi
BE3_DOMAIN_NAME="$address" BE3_BACKEND_URL="$backend" BE3_WEB_ROOT="$(cd "$bundle" && pwd)" \
    "$caddy" run --adapter caddyfile --config "$caddyfile"
