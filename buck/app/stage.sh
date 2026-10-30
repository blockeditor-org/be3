#!/bin/sh
#
# Lays out the app as it runs: the executable under cargo's name, --file beside
# it as it is, and each plugin's manifest renamed <id>.plugin.json with
# the module it names and its .cwasm, and --data, the plugin's read-only data
# (plugin_data), under data/<id>/. Each --library is a library the modules
# are linked against (guides/plugin_libraries.md), staged as lib-<its SHA-256>.wasm,
# which is the name a module finds it by, with --library-artifact, its .cwasm.
# --index writes plugins.json,
# the list of manifests the app finds its plugins through. For the web bundle,
# --tree copies in what wasm-bindgen wrote. --plugin-type, after a plugin's
# MANIFEST=MODULE, stages that plugin's type module (guides/type_modules.md)
# under the name its manifest's block_types give it, with its SHA-256 as
# NAME.sha256 (the tag of its sessions) and, with --type-artifact, its .cwasm.
# --compiled-only leaves out each module that has a .cwasm, for the APK,
# which only falls back to one.
#
# Usage:
#   stage.sh OUT [--executable=EXECUTABLE=NAME] [--file=FILE]... [--tree=DIR]...
#            [--index] [--compiled-only] [--library=MODULE [--library-artifact=CWASM]]...
#            (MANIFEST=MODULE [--data=DIR] [--artifact=CWASM]
#             [--plugin-type=MODULE [--type-artifact=CWASM]])...
set -eu
out="$1"
shift
mkdir -p "$out"
index=false
compiled_only=false
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
        --compiled-only)
            compiled_only=true
            ;;
        --library=*)
            library="${argument#--library=}"
            identity="$(sha256sum "$library" | cut -d ' ' -f 1)"
            staged="$out/lib-$identity.wasm"
            cp "$library" "$staged"
            ;;
        --library-artifact=*)
            cp "${argument#--library-artifact=}" "${staged%.wasm}.cwasm"
            if $compiled_only; then
                rm "$staged"
            fi
            ;;
        --plugin-type=*)
            type_module="${argument#--plugin-type=}"
            named="$(field "$manifest" module)"
            if [ "$(basename "$type_module")" != "$named" ]; then
                echo "$manifest names $named as its type module, but its type module is $type_module" >&2
                exit 1
            fi
            staged="$out/$named"
            cp "$type_module" "$staged"
            sha256sum "$staged" | cut -d ' ' -f 1 > "${staged%.wasm}.sha256"
            ;;
        --type-artifact=*)
            cp "${argument#--type-artifact=}" "${staged%.wasm}.cwasm"
            if $compiled_only; then
                rm "$staged"
            fi
            ;;
        --data=*)
            mkdir -p "$out/data/$id"
            cp -R "${argument#--data=}/." "$out/data/$id/"
            ;;
        --artifact=*)
            cp "${argument#--artifact=}" "${staged%.wasm}.cwasm"
            if $compiled_only; then
                rm "$staged"
            fi
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
