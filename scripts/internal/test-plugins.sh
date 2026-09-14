#!/usr/bin/env bash
#
# Builds every plugin's tests for WebAssembly and runs them with cargo nextest
# through the same host the app runs a plugin in.
#
# A plugin is a wasm guest wherever it runs, and what it paints depends on the
# FreeType and HarfBuzz it was compiled against. Running its tests natively
# paints with whatever those libraries happen to be on the machine, so the same
# editor paints slightly different pixels from one machine to the next and the
# accepted paintings never settle. Compiled to wasm they are the versions the
# plugin ships with, and the painting is the one the app would show.
#
# nextest runs every test in a process of its own, so each test module is
# compiled by Cranelift once beforehand, several at a time, and left beside the
# module as a .cwasm the way a build leaves one beside a plugin. Each test's
# process then maps that in rather than compiling the module again.
#
# Arguments after --check go to cargo nextest run. A package named with -p
# narrows the run to it; otherwise every plugin is tested.
#
# With --build-only everything a test needs is prepared and nothing is run,
# which is what scripts/internal/warm-cache.sh wants: the sysroot, the runner,
# the test modules and the artifacts Cranelift compiles them to.
#
# Usage:
#   test-plugins.sh [nextest arguments...]            accepts whatever the tests paint
#   test-plugins.sh --check [nextest arguments...]    reports a changed painting instead
#   test-plugins.sh --build-only [-p plugin...]        compiles the tests without running them

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

check=false
build_only=false
case "${1:-}" in
    --check)
        check=true
        shift
        ;;
    --build-only)
        build_only=true
        shift
        ;;
esac

assert_command cargo 'Install Rust from https://rustup.rs.'
cd "$repository"
load_plugins

# The runner is what nextest starts in place of each test binary: it hands the
# module to wasmtime with a plugin's imports linked, and opens a graphics
# device only if a test calls the gpu abi. It is built optimised
# because a debug Cranelift spends minutes on a module this size.
cargo build --quiet --release -p plugin-test-runner
runner="$repository/target/release/plugin-test-runner"
if [[ -f "$runner.exe" ]]; then
    runner+='.exe'
fi
if [[ ! -f "$runner" ]]; then
    echo "cargo did not produce $runner" >&2
    exit 1
fi

packages=()
expect_package=false
for argument in "$@"; do
    if $expect_package; then
        packages+=(-p "$argument")
        expect_package=false
        continue
    fi
    case "$argument" in
        -p | --package) expect_package=true ;;
        --package=*) packages+=(-p "${argument#--package=}") ;;
        -p?*) packages+=(-p "${argument#-p}") ;;
        --) break ;;
    esac
done
selection=()
if [[ ${#packages[@]} -eq 0 ]]; then
    for plugin in "${plugins[@]}"; do
        selection+=(-p "$plugin")
    done
    packages=("${selection[@]}")
fi

build=(--cargo-profile plugin --target "$wasm_rust_target")

(
    export_wasi_toolchain "${wasi_sysroot:-}" > /dev/null
    export "CARGO_TARGET_$(echo "$wasm_rust_target" | tr 'a-z-' 'A-Z_')_RUNNER=$runner"

    listing="$(cargo nextest list --cargo-quiet --list-type binaries-only --message-format json "${build[@]}" "${packages[@]}")"
    stale=()
    while IFS= read -r module; do
        if [[ "$module" == *.wasm && ! "${module%.wasm}.cwasm" -nt "$module" ]]; then
            stale+=("$module")
        fi
    done < <(grep -o '"binary-path":"[^"]*"' <<< "$listing" | sed 's/^"binary-path":"//; s/"$//')
    if [[ ${#stale[@]} -gt 0 ]]; then
        echo "Compiling ${#stale[@]} plugin test modules..."
        "$runner" --precompile "${stale[@]}"
    fi

    if $build_only; then
        exit 0
    fi

    if ! $check; then
        export UPDATE_SNAPSHOTS=1
    fi
    cargo nextest run --no-fail-fast "${build[@]}" "${selection[@]}" "$@"
)
