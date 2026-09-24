#!/usr/bin/env bash
#
# Builds every plugin to WebAssembly through buck2, and lays each out beside its
# manifest, named after the plugin's id: //crates/block-app:plugins.
#
# A plugin is a WASI module, and the same bytes are what Windows, macOS, Linux,
# Android and the browser all load, so CI builds them once here and ships the
# directory beside every platform's app.
#
# Usage:
#   build-plugins.sh [--release] [--output DIRECTORY]

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

profile='debug'
output=''
while [[ $# -gt 0 ]]; do
    case "$1" in
        --release)
            profile='release'
            shift
            ;;
        --output)
            output="$2"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

buck_arguments=()
if [[ "$profile" == 'release' ]]; then
    buck_arguments+=(-c be3.profile=release)
fi

cd "$repository"
time_script 'The plugin build'
output="${output:-$repository/target/plugins/$profile}"

step 'Building the plugins on BuildBuddy'
log="$(mktemp)"
if ! "$repository/scripts/buck" build "${buck_arguments[@]}" //crates/block-app:plugins --show-full-output > "$log" 2>&1; then
    cat "$log" >&2
    rm -f "$log"
    exit 1
fi
plugins="$(awk '$1 == "root//crates/block-app:plugins" { print $2 }' "$log")"
rm -f "$log"
end_step

mkdir -p "$output"
rm -f "$output"/*.wasm "$output"/*.plugin.json
cp -f "$plugins"/* "$output/"
echo "Built $(find "$output" -name '*.plugin.json' | wc -l) plugins in $output"
