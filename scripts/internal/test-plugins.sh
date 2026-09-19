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
# narrows the run to it; otherwise every plugin is tested. --cargo-quiet goes
# to nextest and is also taken as a request to quieten the cargo calls this
# script makes of its own, which is what ./scripts/verify asks for on a
# person's run and never in CI: see the step comment below.
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

# What this prints, and why the cargo calls below are loud unless asked not to
# be. Nearly all of a plugin test run is building: the runner, the test modules
# and the artifacts Cranelift compiles them to, each of them minutes on a cold
# machine and none of them saying anything while they work. Quiet, a run that
# took eleven of them leaves a log with a gap in the middle and no way to tell
# which of the three it was spent in. So every phase announces itself and
# reports what it cost, and a person who wants the tests rather than the
# builds asks for --cargo-quiet.
quiet=false
for argument in "$@"; do
    if [[ "$argument" == '--cargo-quiet' ]]; then
        quiet=true
    fi
done
cargo_quiet=()
nextest_quiet=()
if $quiet; then
    cargo_quiet=(--quiet)
    nextest_quiet=(--cargo-quiet)
fi

# The runner is what nextest starts in place of each test binary: it hands the
# module to wasmtime with a plugin's imports linked, and opens a graphics
# device only if a test calls the gpu abi. Cranelift is what makes that take
# seconds rather than minutes on a module this size, and the workspace profile
# already builds Cranelift optimised, so the runner needs no profile of its own.
#
# What it does need is the selection the native test run uses, plugins excluded
# and --tests on. Cargo resolves features over the packages a call selects and
# over the kinds of dependency it is about to build, so asking for this package
# alone, or for a plain build rather than a test one, unifies them differently
# and compiles wasmtime, wgpu and everything under them a second time. Asked
# for the way the native run asks, every artifact it left behind is reused and
# only the runner itself is linked.
native_selection
step 'Building the plugin test runner'
cargo build "${cargo_quiet[@]}" "${selection[@]}" --bin plugin-test-runner --tests
end_step
runner="$repository/target/debug/plugin-test-runner"
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
# packages holds a -p and a name for each one, so what the steps below report
# is how many plugins that is rather than how many arguments it took to say so.
plugin_count=$((${#packages[@]} / 2))

build=(--cargo-profile plugin --target "$wasm_rust_target")

(
    if $quiet; then
        export_wasi_toolchain "${wasi_sysroot:-}" > /dev/null
    else
        export_wasi_toolchain "${wasi_sysroot:-}"
    fi
    export "CARGO_TARGET_$(echo "$wasm_rust_target" | tr 'a-z-' 'A-Z_')_RUNNER=$runner"

    # Listing the test binaries is what builds them, so this is the compile of
    # every plugin's tests for wasm and not the bookkeeping its name suggests.
    step "Building the test modules for $plugin_count plugins"
    listing="$(cargo nextest list "${nextest_quiet[@]}" --list-type binaries-only --message-format json "${build[@]}" "${packages[@]}")"
    end_step
    stale=()
    while IFS= read -r module; do
        if [[ "$module" == *.wasm && ! "${module%.wasm}.cwasm" -nt "$module" ]]; then
            stale+=("$module")
        fi
    done < <(grep -o '"binary-path":"[^"]*"' <<< "$listing" | sed 's/^"binary-path":"//; s/"$//')
    if [[ ${#stale[@]} -gt 0 ]]; then
        step "Compiling ${#stale[@]} plugin test modules"
        "$runner" --precompile "${stale[@]}"
        end_step
    fi

    if $build_only; then
        exit 0
    fi

    if ! $check; then
        export UPDATE_SNAPSHOTS=1
    fi
    step "Running the tests of $plugin_count plugins"
    cargo nextest run --no-fail-fast "${build[@]}" "${selection[@]}" "$@"
    end_step
)
