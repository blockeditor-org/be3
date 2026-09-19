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
#   test-plugins.sh --runner-only [arguments...]      builds only the runner, not the
#                                                     workspace's tests: see the comment on
#                                                     the runner build below
#   test-plugins.sh --build-only [-p plugin...]        compiles the tests without running them

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

check=false
build_only=false
runner_only=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --check)
            check=true
            shift
            ;;
        --build-only)
            build_only=true
            shift
            ;;
        --runner-only)
            runner_only=true
            shift
            ;;
        # Everything from the first argument this does not know belongs to
        # nextest, including one that happens to be spelled like these.
        *) break ;;
    esac
done

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
# What it does need, when the native test run is part of the same verification,
# is the selection that run uses: plugins excluded and --tests on. Cargo
# resolves features over the packages a call selects and over the kinds of
# dependency it is about to build, so asking for this package alone, or for a
# plain build rather than a test one, unifies them differently and compiles
# wasmtime, wgpu and everything under them a second time. Asked for the way the
# native run asks, every artifact it left behind is reused and only the runner
# itself is linked.
#
# --runner-only is for when there is no such run to share with, which is how CI
# runs this: on a runner of its own, --tests is a second build of every test
# binary in the workspace for the sake of one bin. Measured cold, it is 571
# crates and 44 linked binaries against 246 and one, and six and a half minutes
# against four.
#
# What the two resolutions differ by is one feature: a dev-dependency asks wgpu
# for its noop backend, so the runner built the shared way carries five
# megabytes of it. It cannot ever reach it. The noop adapter is handed out only
# when NoopBackendOptions says so, which defaults to off and which nothing here
# sets, and the runner asks wgpu::Instance::default() for a real adapter.
native_selection
runner_targets=(--tests)
if $runner_only; then
    runner_targets=()
fi
step 'Building the plugin test runner'
cargo build "${cargo_quiet[@]}" "${selection[@]}" --bin plugin-test-runner "${runner_targets[@]}"
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
    # An artifact the runner will not map in is one every test process
    # compiles again for itself, which is minutes rather than the seconds
    # compiling it once here costs, and nothing says so: the run merely gets
    # slow. So what counts as stale is what wasmtime would refuse, which is an
    # artifact older than the module or older than the runner that has to read
    # it, the same two questions common.sh asks of a plugin's own .cwasm.
    stale=()
    while IFS= read -r module; do
        artifact="${module%.wasm}.cwasm"
        if [[ "$module" == *.wasm ]] && [[ ! "$artifact" -nt "$module" || ! "$artifact" -nt "$runner" ]]; then
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
