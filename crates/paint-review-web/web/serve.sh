#!/bin/sh
#
# Serves the paint review page with Caddy on http://127.0.0.1:8075, or on the
# address given, laid out as CI uploads it: the bundle at site/dev/ and every
# file in DIRECTORY (CHANGES.json lists and IMAGE.png images) beside site/.
#
#   ./scripts/buck run //crates/paint-review-web:web [-- HOST:PORT [DIRECTORY]]
#
# then open http://HOST:PORT/site/dev/index.html?changes=CHANGES.
set -eu
caddy="$1" caddyfile="$2" bundle="$3"
listen="${4:-127.0.0.1:8075}"
data="${5:-}"
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
echo "Serving http://$listen/site/dev/index.html?changes=CHANGES"
PAINT_REVIEW_ADDRESS="http://$listen" PAINT_REVIEW_ROOT="$root" \
    "$caddy" run --adapter caddyfile --config "$caddyfile"
