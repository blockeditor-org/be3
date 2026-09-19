#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

wasi_sysroot=''
profile='debug'
with_plugins=true
while [[ $# -gt 0 ]]; do
    case "$1" in
        --wasi-sysroot)
            wasi_sysroot="$2"
            shift 2
            ;;
        --release)
            profile='release'
            shift
            ;;
        --no-plugins)
            with_plugins=false
            shift
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

assert_command cargo 'Install Rust from https://rustup.rs.'
assert_command clang 'Install LLVM and put its bin directory on PATH.'
assert_command llvm-ar 'Install LLVM and put its bin directory on PATH.'
cd "$repository"
time_script 'The web build'

output_directory="$repository/target/web"
# Threads, so that the app and its plugins can move work off the thread that
# draws. The target gives the module a shared memory and wasi-libc's pthreads,
# which reach the browser through the wasi-threads import that web/threads.js
# answers by starting a Worker on the very same module and memory.
rust_target="$wasm_rust_target"
wasm_bindgen_version='0.2.122'
cargo_home="${CARGO_HOME:-$HOME/.cargo}"
wasm_bindgen="$cargo_home/bin/wasm-bindgen"

if [[ ! -x "$wasm_bindgen" ]]; then
    step "Installing wasm-bindgen-cli $wasm_bindgen_version"
    cargo install wasm-bindgen-cli --version "$wasm_bindgen_version"
    end_step
fi
if [[ ! -x "$wasm_bindgen" ]]; then
    echo "wasm-bindgen was not installed at $wasm_bindgen" >&2
    exit 1
fi

cargo_arguments=()
if [[ "$profile" == 'release' ]]; then
    cargo_arguments+=(--release)
fi

# The bundle is the whole of what is served, so it is rebuilt from nothing
# rather than left holding what a plugin that has since gone produced.
rm -rf "$output_directory"
mkdir -p "$output_directory"

# The games are compiled for a target of their own, before the WASI toolchain
# below is exported over the environment the rest of the build runs in. Nothing
# in the bundle is a game, so this only says they still compile, which is what
# --no-plugins leaves to the build that produces the modules.
if $with_plugins; then
    build_games "$profile"
fi

# The app is a WASI module too, so it is built against the same sysroot a
# plugin is. The plugins below export this again for a cargo call of their own,
# so where the sysroot comes from and what it is linked with lives in one place
# rather than being spelled out again there.
export_wasi_toolchain "$wasi_sysroot"

# The terminal emulator the debug terminal window is built on, as a freestanding
# WebAssembly archive the app's own module links in. Only a full build has a
# terminal, so only a full build needs Zig to cross-compile one.
ensure_ghostty_vt "$rust_target"

app_features=()
if full_build; then
    app_features=(--features block-app/full)
fi
step "Building the app for $rust_target"
cargo build --lib --target "$rust_target" "${cargo_arguments[@]}" -p block-app "${app_features[@]}"
end_step

# The shim is the only other module the browser needs bindings for. It holds a
# real wgpu device on the plugin's canvas and answers the gpu abi from it.
step 'Building the gpu shim for wasm32-unknown-unknown'
if ! rustup target list --installed | grep -qx 'wasm32-unknown-unknown'; then
    echo 'Installing the wasm32-unknown-unknown Rust target...'
    rustup target add wasm32-unknown-unknown
fi
(
    unset RUSTFLAGS
    cargo build --lib --target wasm32-unknown-unknown "${cargo_arguments[@]}" -p block-gpu-shim
)
end_step

modules_directory="$repository/target/$rust_target/$profile"

# wasm-bindgen holds the whole module in memory, and a debug build of the app
# or of a plugin is hundreds of megabytes, so how many run at once is bounded
# by memory rather than by cores.
bindgen_jobs=2
bindgen_pids=()
bindgen_modules=()

await_bindings() {
    local index failed=false
    for index in "${!bindgen_pids[@]}"; do
        if ! wait "${bindgen_pids[$index]}"; then
            echo "wasm-bindgen failed for ${bindgen_modules[$index]}" >&2
            failed=true
        fi
    done
    bindgen_pids=()
    bindgen_modules=()
    if $failed; then
        exit 1
    fi
}

generate() {
    generate_from "$modules_directory" "$1"
}

generate_from() {
    local module="$1/$2.wasm"
    if [[ ! -f "$module" ]]; then
        echo "cargo did not produce $module" >&2
        exit 1
    fi
    if [[ ${#bindgen_pids[@]} -ge $bindgen_jobs ]]; then
        await_bindings
    fi
    "$wasm_bindgen" --target web --no-typescript --out-dir "$output_directory" "$module" &
    bindgen_pids+=($!)
    bindgen_modules+=("$module")
}

step 'Generating JavaScript bindings'
generate block_app_lib
generate_from "$repository/target/wasm32-unknown-unknown/$profile" block_gpu_shim
await_bindings
end_step

# The app links wgpu's real backends and a plugin links only the custom one, so
# they are separate cargo calls; a single call would unify the two and leave the
# guest carrying a backend it cannot use.
#
# The modules are the same bytes every platform loads, so --no-plugins leaves
# them to the one build that produces them for all of them rather than
# compiling them again here.
if $with_plugins; then
    load_plugins
    build_plugin_wasm "$profile" "$output_directory"
    stage_plugin_manifests "$output_directory"
    write_plugin_index "$output_directory"
else
    # An index of nothing rather than no index. Discovery reports a bundle
    # whose plugins.json it cannot fetch as an error, and a bundle built
    # without plugins has none to find rather than something wrong with it.
    write_index "$output_directory/plugins.json"
fi

step 'Assembling the bundle'
cp "$internal/web/index.html" "$output_directory"
cp "$internal/web/plugin.js" "$output_directory"
cp "$internal/web/wasi.js" "$output_directory"
cp "$internal/web/env.js" "$output_directory"
cp "$internal/web/threads.js" "$output_directory"
cp "$internal/web/thread.js" "$output_directory"
end_step

size="$(du -h "$output_directory/block_app_lib_bg.wasm" | cut -f1)"
printf '\nWrote %s (%s of WebAssembly).\n' "$output_directory" "$size"
