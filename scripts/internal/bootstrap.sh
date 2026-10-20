#!/bin/sh
#
# What every script in scripts/ runs: crates/bootstrap's release for this
# machine, as scripts/internal/pins names it, downloaded into
# target/tools/bootstrap-<sha256>/ and checked against that hash the first
# time, then run with the script's arguments (guides/bootstrap.md). This is
# the only shell the scripts need; everything else is the bootstrap.
#
# Usage: sh scripts/internal/bootstrap.sh COMMAND [ARGUMENTS...]

set -eu

repository="$(cd "$(dirname "$0")/../.." && pwd)"

case "$(uname -m)" in
    x86_64 | amd64) architecture='x86_64' ;;
    aarch64 | arm64) architecture='aarch64' ;;
    *) echo "No bootstrap is released for $(uname -m)." >&2; exit 1 ;;
esac
suffix=''
case "$(uname -s)" in
    Linux) triple="$architecture-unknown-linux-gnu" ;;
    Darwin) triple="$architecture-apple-darwin" ;;
    MINGW* | MSYS* | CYGWIN*) triple="$architecture-pc-windows-msvc"; suffix='.exe' ;;
    *) echo "No bootstrap is released for $(uname -s)." >&2; exit 1 ;;
esac

name="bootstrap_sha256_$(echo "$triple" | tr '-' '_')"
sha256="$(sed -n "s/^$name='\([0-9a-f]*\)'\$/\1/p" "$repository/scripts/internal/pins")"
if [ -z "$sha256" ]; then
    echo "scripts/internal/pins names no bootstrap release for $triple." >&2
    exit 1
fi

binary="$repository/target/tools/bootstrap-$sha256/bootstrap$suffix"
if [ ! -x "$binary" ]; then
    url="https://be3-ci.b-cdn.net/bootstrap/$sha256"
    partial="$binary.$$.partial"
    if ! command -v curl > /dev/null 2>&1; then
        echo 'curl was not found on PATH. Install curl.' >&2
        exit 1
    fi
    echo "Downloading the bootstrap from $url..." >&2
    mkdir -p "$(dirname "$binary")"
    curl --fail --location --proto '=https' --proto-redir '=https' --silent --show-error \
        --connect-timeout 30 --retry 4 --retry-connrefused --output "$partial" "$url"
    if command -v sha256sum > /dev/null 2>&1; then
        actual="$(sha256sum "$partial" | cut -d ' ' -f 1)"
    else
        actual="$(shasum -a 256 "$partial" | cut -d ' ' -f 1)"
    fi
    if [ "$actual" != "$sha256" ]; then
        rm -f "$partial"
        echo "The download from $url is not the bootstrap scripts/internal/pins names:" >&2
        echo "  sha256 is   $actual" >&2
        echo "  sha256 want $sha256" >&2
        exit 1
    fi
    chmod +x "$partial"
    mv -f "$partial" "$binary"
fi

exec "$binary" "$@"
