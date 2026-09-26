#!/bin/sh
# Serves the web build and opens it in a headless Chromium for an agent to
# drive, signed in to an account on its own be-server with a workspace open,
# and returns once the page is up. guides/running_the_web_app.md says how to
# drive it.
#
#   ./scripts/buck run //crates/block-app:web-dev              start, or restart
#   ./scripts/buck run //crates/block-app:web-dev -- --fresh   start with no data
#   ./scripts/buck run //crates/block-app:web-dev -- --webgl   start without WebGPU
#   ./scripts/buck run //crates/block-app:web-dev -- --stop    stop it
set -u
caddy="$1" caddyfile="$2" bundle="$3" server="$4" drive="$5"
shift 5
fresh=false
stop=false
webgl=false
for argument in "$@"; do
    case "$argument" in
        --fresh) fresh=true ;;
        --stop) stop=true ;;
        --webgl) webgl=true ;;
        *) echo "Unknown argument $argument; expected --fresh, --webgl or --stop." >&2; exit 1 ;;
    esac
done

dir="${BLOCK_WEB_DEV_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/be3/web-dev}"
port="${BLOCK_WEB_DEV_PORT:-8090}"
backend=$((port + 1))
debugging=$((port + 2))
admin=$((port + 3))
mkdir -p "$dir"

stop_pid() {
    [ -f "$dir/$1.pid" ] || return 0
    pid="$(cat "$dir/$1.pid")"
    if kill "$pid" 2> /dev/null; then
        while kill -0 "$pid" 2> /dev/null; do sleep 0.1; done
    fi
    rm -f "$dir/$1.pid"
}

if [ -f "$dir/browser.pid" ] && kill -0 "$(cat "$dir/browser.pid")" 2> /dev/null; then
    (. "$dir/env" && drive quit) > /dev/null 2>&1
fi
stop_pid browser
stop_pid caddy
stop_pid server
if $stop; then
    echo 'Stopped the browser and the servers.'
    exit 0
fi

command -v node > /dev/null || { echo 'Missing node, which drives the browser.' >&2; exit 1; }
NODE_PATH="$(npm root -g 2> /dev/null)"
export NODE_PATH
chromium="$(node -e "console.log(require('playwright').chromium.executablePath())" 2> /dev/null)"
if [ -z "$chromium" ] || ! [ -x "$chromium" ]; then
    echo 'Missing Playwright and its Chromium: npm install -g playwright && npx playwright install chromium' >&2
    exit 1
fi

if $fresh; then
    rm -rf "$dir/server" "$dir/profile"
fi

[ -x "$caddy/caddy" ] && caddy="$caddy/caddy" || caddy="$caddy/caddy.exe"
setsid "$server" --addr "127.0.0.1:$backend" --data-dir "$dir/server" \
    > "$dir/server.log" 2>&1 < /dev/null &
echo $! > "$dir/server.pid"
BE3_DOMAIN_NAME="http://127.0.0.1:$port" BE3_BACKEND_URL="127.0.0.1:$backend" \
    BE3_WEB_ROOT="$(cd "$bundle" && pwd)" CADDY_ADMIN="127.0.0.1:$admin" \
    setsid "$caddy" run --adapter caddyfile --config "$caddyfile" \
    > "$dir/caddy.log" 2>&1 < /dev/null &
echo $! > "$dir/caddy.pid"
url="http://127.0.0.1:$port/"
if ! timeout 30 sh -c "until curl -sf -o /dev/null '$url'; do sleep 0.1; done"; then
    echo "The page was not served; see $dir/caddy.log and $dir/server.log." >&2
    exit 1
fi

if $webgl; then
    graphics='--disable-features=WebGPU --use-angle=swiftshader'
else
    graphics='--enable-unsafe-webgpu --enable-features=Vulkan,WebGPUService --use-angle=vulkan --use-vulkan=swiftshader'
fi
# shellcheck disable=SC2086
setsid "$chromium" --headless=new --no-sandbox --no-first-run --no-default-browser-check \
    $graphics --enable-unsafe-swiftshader --ignore-gpu-blocklist \
    --enable-logging=stderr --v=0 --window-size=1280,800 \
    --remote-debugging-port="$debugging" --user-data-dir="$dir/profile" \
    "${url}?dev-workspace&accessibility-tree" > "$dir/browser.log" 2>&1 < /dev/null &
echo $! > "$dir/browser.pid"

cat > "$dir/env" <<ENV
export BLOCK_WEB_DEV=http://127.0.0.1:$debugging
export NODE_PATH=$NODE_PATH
drive() { node "$drive" "\$@"; }
ENV
. "$dir/env"
if ! timeout 60 sh -c ". '$dir/env'; until drive tree 2> /dev/null | grep -q '^Window'; do sleep 0.2; done"; then
    echo "The page did not start; see $dir/browser.log." >&2
    exit 1
fi
drive size 1280 800
cat <<INFO
The web app is served on $url and open in a headless Chromium;
guides/running_the_web_app.md says how to drive it.
  source $dir/env    defines drive
  logs: $dir/browser.log (the page's and its workers' consoles), $dir/server.log
INFO
