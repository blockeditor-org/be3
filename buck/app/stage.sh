#!/bin/sh
#
# Lays out the app as it runs: the executable under cargo's name, --file beside
# it as it is (PDFium), and each plugin's manifest renamed <id>.plugin.json with
# the module it names and its .cwasm. For the web bundle, --tree copies in what
# wasm-bindgen wrote and --index writes plugins.json.
#
# Usage:
#   stage.sh OUT [--executable=EXECUTABLE=NAME] [--file=FILE]... [--tree=DIR]...
#            [--index] (MANIFEST=MODULE [--artifact=CWASM])...
set -eu
out="$1"
shift
mkdir -p "$out"
index=false
manifests=''
staged=''

field() {
    sed -n "s/^[[:space:]]*\"$2\"[[:space:]]*:[[:space:]]*\"\\([^\"]*\\)\".*/\\1/p" "$1" | head -n 1
}

for argument in "$@"; do
    case "$argument" in
        --executable=*)
            pair="${argument#--executable=}"
            cp "${pair%%=*}" "$out/${pair#*=}"
            chmod +x "$out/${pair#*=}"
            ;;
        --file=*)
            cp "${argument#--file=}" "$out/"
            ;;
        --tree=*)
            cp -R "${argument#--tree=}/." "$out/"
            ;;
        --index)
            index=true
            ;;
        --artifact=*)
            cp "${argument#--artifact=}" "${staged%.wasm}.cwasm"
            ;;
        *)
            manifest="${argument%%=*}"
            module="${argument#*=}"
            id="$(field "$manifest" id)"
            entry_point="$(field "$manifest" entry_point)"
            if [ "$(basename "$module")" != "$entry_point" ]; then
                echo "$manifest names $entry_point as its entry point, but its module is $module" >&2
                exit 1
            fi
            cp "$manifest" "$out/$id.plugin.json"
            manifests="$manifests $id.plugin.json"
            staged="$out/$entry_point"
            cp "$module" "$staged"
            ;;
    esac
done

if $index; then
    {
        printf '['
        separator=''
        for name in $manifests; do
            printf '%s\n  "%s"' "$separator" "$name"
            separator=','
        done
        [ -n "$manifests" ] && printf '\n'
        printf ']\n'
    } > "$out/plugins.json"
fi
