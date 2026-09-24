#!/usr/bin/env bash
#
# Builds the app and the server for one platform through buck2, and lays them
# out the way they run: the executables, PDFium, and every plugin beside the
# app. Every platform is built on BuildBuddy's Linux workers, this one's and the
# others' alike; guides/buck2.md says how each is cross-compiled.
#
# The output is target/native/TRIPLE/PROFILE unless --output names another
# directory. --no-plugins leaves the plugins out, for a build that ships beside
# the one plugins directory every platform shares.
#
# Usage:
#   build-native.sh [--triple TRIPLE] [--release] [--output DIRECTORY]
#                   [--no-client] [--no-server] [--no-plugins]
#                   [--sign-identity IDENTITY]

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

triple='x86_64-unknown-linux-gnu'
profile='debug'
output=''
client=true
server=true
with_plugins=true
sign_identity=''
while [[ $# -gt 0 ]]; do
    case "$1" in
        --triple)
            triple="$2"
            shift 2
            ;;
        --release)
            profile='release'
            shift
            ;;
        --output)
            output="$2"
            shift 2
            ;;
        --no-client)
            client=false
            shift
            ;;
        --no-server)
            server=false
            shift
            ;;
        --no-plugins)
            with_plugins=false
            shift
            ;;
        --sign-identity)
            sign_identity="$2"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

# The buck2 platform each triple is built as; buck/platforms/cross.bzl lists
# them. The host is Linux on x86_64, and needs none.
case "$triple" in
    x86_64-unknown-linux-gnu) platform='' ;;
    aarch64-unknown-linux-gnu) platform='linux_arm64' ;;
    aarch64-apple-darwin) platform='macos_arm64' ;;
    x86_64-apple-darwin) platform='macos_x86_64' ;;
    aarch64-pc-windows-msvc) platform='windows_arm64' ;;
    x86_64-pc-windows-msvc) platform='windows_x86_64' ;;
    *)
        echo "No buck2 platform builds $triple" >&2
        exit 1
        ;;
esac

extension=''
case "$triple" in
    *-windows-*) extension='.exe' ;;
esac

# A signing identity is codesign's, which only a Mac has. lld already signs a
# macOS binary ad hoc, so an unsigned build still runs.
if [[ -n "$sign_identity" ]]; then
    case "$triple" in
        *-apple-darwin) assert_command codesign 'macOS signing requires the Xcode command-line tools.' ;;
        *)
            echo '--sign-identity is only valid for macOS targets' >&2
            exit 1
            ;;
    esac
fi

targets=()
if $client; then
    targets+=(//crates/block-app:app)
fi
if $server; then
    targets+=(//crates/block-server:block-server-bin)
fi
if [[ ${#targets[@]} -eq 0 ]]; then
    echo '--no-client and --no-server together leave nothing to build' >&2
    exit 1
fi

buck_arguments=()
if [[ -n "$platform" ]]; then
    buck_arguments+=(--target-platforms "root//buck/platforms:$platform")
fi
if [[ "$profile" == 'release' ]]; then
    buck_arguments+=(-c be3.profile=release)
fi

cd "$repository"
time_script 'The native build'
output="${output:-$repository/target/native/$triple/$profile}"

step "Building $triple ($profile) on BuildBuddy"
log="$(mktemp)"
if ! "$repository/scripts/buck" build "${buck_arguments[@]}" "${targets[@]}" --show-full-output > "$log" 2>&1; then
    cat "$log" >&2
    rm -f "$log"
    exit 1
fi
built() {
    awk -v target="$1" '$1 == target { print $2 }' "$log"
}
end_step

step "Laying $triple out in $output"
mkdir -p "$output"
if $client; then
    app="$(built root//crates/block-app:app)"
    rm -f "$output"/*.plugin.json "$output"/*.wasm "$output"/*.cwasm
    for file in "$app"/*; do
        case "$file" in
            *.plugin.json | *.wasm | *.cwasm) $with_plugins || continue ;;
        esac
        cp -f "$file" "$output/"
    done
fi
if $server; then
    cp -f "$(built root//crates/block-server:block-server-bin)" "$output/block-server$extension"
fi
rm -f "$log"
end_step

if [[ -n "$sign_identity" ]]; then
    for executable in "$output/block-app" "$output/block-server"; do
        if [[ -f "$executable" ]]; then
            codesign --force --options runtime --sign "$sign_identity" "$executable"
        fi
    done
fi

echo "Built $triple in $output"
