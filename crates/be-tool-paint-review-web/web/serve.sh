#!/bin/sh
#
# Serves the paint review page with Caddy on http://127.0.0.1:8075, or on the
# address given with --addr, laid out as CI uploads it: the bundle at site/dev/
# and every file in DIRECTORY (CHANGES.json lists and IMAGE.webp images)
# beside site/. Caddy listens only on HOST; `--addr 0.0.0.0:PORT` is how to
# reach it from another machine (guides/hosting.md, "Listening").
#
#   ./scripts/buck run //crates/be-tool-paint-review-web:web [-- [--addr HOST:PORT] [DIRECTORY]]
#
# then open http://HOST:PORT/site/dev/index.html?changes=CHANGES.
set -eu
caddy="$1" caddyfile="$2" bundle="$3"
shift 3
address=127.0.0.1:8075
data=
while [ $# -gt 0 ]; do
    case "$1" in
        --addr) [ $# -ge 2 ] || { echo "--addr needs HOST:PORT." >&2; exit 1; }; address="$2"; shift 2 ;;
        -*) echo "Unknown argument $1; expected --addr HOST:PORT or a directory." >&2; exit 1 ;;
        *) data="$1"; shift ;;
    esac
done
case "$address" in
    *:*) ;;
    *) echo "--addr takes HOST:PORT, such as 127.0.0.1:8080, not $address." >&2; exit 1 ;;
esac
[ -x "$caddy/caddy" ] && caddy="$caddy/caddy" || caddy="$caddy/caddy.exe"
root="$(mktemp -d)"
trap 'rm -rf "$root"' EXIT INT TERM
mkdir "$root/site"
ln -s "$(cd "$bundle" && pwd)" "$root/site/dev"
if [ -n "$data" ]; then
    data="$(cd "$data" && pwd)"
    for file in "$data"/*; do
        ln -s "$file" "$root/"
    done
fi
echo "Serving http://$address/site/dev/index.html?changes=CHANGES"
PAINT_REVIEW_BIND="${address%:*}" PAINT_REVIEW_PORT="${address##*:}" PAINT_REVIEW_ROOT="$root" \
    "$caddy" run --adapter caddyfile --config "$caddyfile"
