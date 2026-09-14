#!/usr/bin/env bash
#
# Builds every plugin's tests for WebAssembly and runs them through the same
# host the app runs a plugin in.
#
# A plugin is a wasm guest wherever it runs, and what it paints depends on the
# FreeType and HarfBuzz it was compiled against. Running its tests natively
# paints with whatever those libraries happen to be on the machine, so the same
# editor paints slightly different pixels from one machine to the next and the
# accepted paintings never settle. Compiled to wasm they are the versions the
# plugin ships with, and the painting is the one the app would show.
#
# Usage:
#   test-plugins.sh            accepts whatever the tests paint
#   test-plugins.sh --check    reports a changed painting instead

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

check=false
if [[ "${1:-}" == '--check' ]]; then
    check=true
    shift
fi

assert_command cargo 'Install Rust from https://rustup.rs.'
cd "$repository"
load_plugins

# The runner is what cargo starts in place of each test binary: it hands the
# module to wasmtime with the plugin's gpu abi linked, so the tests paint
# through the host's device exactly as a plugin does. It is built optimised
# because a debug Cranelift spends minutes on a module this size.
echo 'Building the plugin test runner...'
cargo build --release -p plugin-test-runner
runner="$repository/target/release/plugin-test-runner"
if [[ -f "$runner.exe" ]]; then
    runner+='.exe'
fi
if [[ ! -f "$runner" ]]; then
    echo "cargo did not produce $runner" >&2
    exit 1
fi

selection=()
for plugin in "${plugins[@]}"; do
    selection+=(-p "$plugin")
done

echo "Testing ${#plugins[@]} plugins on $wasm_rust_target..."
(
    export_wasi_toolchain "${wasi_sysroot:-}"
    export "CARGO_TARGET_$(echo "$wasm_rust_target" | tr 'a-z-' 'A-Z_')_RUNNER=$runner"
    if ! $check; then
        export UPDATE_SNAPSHOTS=1
    fi
    cargo test --no-fail-fast --profile plugin --target "$wasm_rust_target" "${selection[@]}" "$@"
)
