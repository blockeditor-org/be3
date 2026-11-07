#!/bin/sh
#
# What every script in scripts/ runs: crates/be-tool-bootstrap's release for this
# machine, as scripts/internal/pins names it, downloaded into
# target/tools/bootstrap-<sha256>/, checked against that hash, then run with
# the script's arguments (guides/bootstrap.md). With BE3_BOOTSTRAP=cargo it
# builds the crate with cargo instead and runs that, for when no release can
# be downloaded or none is pinned for this machine. This is the only shell the
# scripts need; everything else is the bootstrap.
#
# Usage: sh scripts/internal/bootstrap.sh COMMAND [ARGUMENTS...]

set -eu

repository="$(cd "$(dirname "$0")/../.." && pwd)"

fail() {
    echo "$1" >&2
    echo 'To build the bootstrap here instead, install Rust (rustup) and run the script again with' >&2
    echo 'BE3_BOOTSTRAP=cargo set (guides/bootstrap.md, "Recovery").' >&2
    exit 1
}

if [ "${BE3_BOOTSTRAP:-}" = cargo ]; then
    if ! command -v cargo > /dev/null 2>&1; then
        echo 'BE3_BOOTSTRAP=cargo: cargo was not found on PATH. Install Rust with rustup.' >&2
        exit 1
    fi
    built="$repository/target/tools/bootstrap-cargo"
    cargo build --locked --release --quiet --manifest-path "$repository/Cargo.toml" \
        -p be-tool-bootstrap --target-dir "$built" >&2
    exec "$built/release/bootstrap" "$@"
fi

case "$(uname -m)" in
    x86_64 | amd64) architecture='x86_64' ;;
    aarch64 | arm64) architecture='aarch64' ;;
    *) fail "No bootstrap is released for $(uname -m)." ;;
esac
suffix=''
case "$(uname -s)" in
    Linux) triple="$architecture-unknown-linux-gnu" ;;
    Darwin) triple="$architecture-apple-darwin" ;;
    MINGW* | MSYS* | CYGWIN*) triple="$architecture-pc-windows-msvc"; suffix='.exe' ;;
    *) fail "No bootstrap is released for $(uname -s)." ;;
esac

name="bootstrap_sha256_$(echo "$triple" | tr '-' '_')"
sha256="$(sed -n "s/^$name='\([0-9a-f]*\)'\$/\1/p" "$repository/scripts/internal/pins")"
if [ -z "$sha256" ]; then
    fail "scripts/internal/pins names no bootstrap release for $triple."
fi

sha256_of() {
    if command -v sha256sum > /dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

# What is already there is checked too, so that a file left or put there is
# never run in the release's place. A mismatch is downloaded again.
binary="$repository/target/tools/bootstrap-$sha256/bootstrap$suffix"
if [ -x "$binary" ] && [ "$(sha256_of "$binary")" != "$sha256" ]; then
    echo "$binary is not the bootstrap scripts/internal/pins names; downloading it again." >&2
    rm -f "$binary"
fi
if [ ! -x "$binary" ]; then
    url="https://be3-ci.b-cdn.net/bootstrap/$sha256"
    partial="$binary.$$.partial"
    if ! command -v curl > /dev/null 2>&1; then
        fail 'curl was not found on PATH. Install curl.'
    fi
    echo "Downloading the bootstrap from $url..." >&2
    mkdir -p "$(dirname "$binary")"
    if ! curl --fail --location --proto '=https' --proto-redir '=https' --silent --show-error \
        --connect-timeout 30 --retry 4 --retry-connrefused --output "$partial" "$url"; then
        rm -f "$partial"
        fail "Could not download the bootstrap from $url."
    fi
    actual="$(sha256_of "$partial")"
    if [ "$actual" != "$sha256" ]; then
        rm -f "$partial"
        fail "The download from $url is not the bootstrap scripts/internal/pins names:
  sha256 is   $actual
  sha256 want $sha256"
    fi
    chmod +x "$partial"
    mv -f "$partial" "$binary"
fi

exec "$binary" "$@"
