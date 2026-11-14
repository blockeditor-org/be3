#!/bin/sh
#
# Serves a web bundle with Caddy (bundle.Caddyfile) on DEFAULT, a loopback
# HOST:PORT, or on the address given with --addr after `--`: what
# `./scripts/buck run` of a web_program with serve (buck/wasm/defs.bzl) runs.
# Caddy listens only on HOST; `--addr 0.0.0.0:PORT` is how to reach it from
# another machine (guides/hosting.md, "Listening").
#
#   serve-bundle.sh CADDY CADDYFILE DEFAULT BUNDLE [--addr HOST:PORT]
set -eu
caddy="$1" caddyfile="$2" address="$3" bundle="$4"
shift 4
case "$*" in
    "") ;;
    "--addr "*) [ $# -eq 2 ] && address="$2" || { echo "Expected --addr HOST:PORT." >&2; exit 1; } ;;
    *) echo "Unknown arguments $*; expected --addr HOST:PORT." >&2; exit 1 ;;
esac
case "$address" in
    *:*) ;;
    *) echo "--addr takes HOST:PORT, such as 127.0.0.1:8080, not $address." >&2; exit 1 ;;
esac
[ -x "$caddy/caddy" ] && caddy="$caddy/caddy" || caddy="$caddy/caddy.exe"
echo "Serving http://$address"
BUNDLE_BIND="${address%:*}" BUNDLE_PORT="${address##*:}" BUNDLE_ROOT="$(cd "$bundle" && pwd)" \
    "$caddy" run --adapter caddyfile --config "$caddyfile"
