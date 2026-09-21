#!/usr/bin/env bash
#
# Compiles everything ./scripts/verify needs, without running a test.
#
# Verify is only slow the first time. On a machine whose target directory is
# already populated it finishes in well under a minute; on a fresh checkout it
# spends over ten minutes compiling before the first test runs, because
# every phase of it wants a different set of artifacts: clippy wants the
# workspace checked with every feature on, the test phase wants it built for
# real, and the plugin phase wants a WASI sysroot and every plugin's tests
# compiled to wasm and handed to Cranelift. The runner Cranelift comes in is
# the one thing it does not add to the pile: it is built from what the test
# phase leaves behind, so it has to be warmed after that phase rather than
# before it.
#
# This builds all of that and stops there. ./scripts/setup runs it when it is
# asked for with --warm-cache, for a machine that would rather wait while it is
# being prepared than in the middle of the first verify; a default setup leaves
# it out, because the compiling happens either way and most of it comes out of
# the shared sccache store. Running it again later is cheap: it is the same
# cargo calls verify makes, so anything still current is left alone.
#
# Usage:
#   warm-cache.sh

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

if [[ $# -ne 0 ]]; then
    echo 'Usage: warm-cache.sh' >&2
    exit 1
fi

assert_command cargo 'Install Rust from https://rustup.rs.'
cd "$repository"

# The plugins are excluded from the native run for the same reason verify
# excludes them: their tests are wasm, and the plugin phase below builds those.
# The selection is otherwise verify's, so what is warmed is what verify asks
# for rather than something close to it.
native_selection
native=("${selection[@]}")

if full_build; then
    echo 'Warming libghostty-vt...'
    host_triple="$(rustc --version --verbose | sed -n 's/^host: //p')"
    ensure_ghostty_vt "$host_triple"
fi

echo 'Warming crates/fix-rust-source...'
cargo build --quiet -p fix-rust-source

# Clippy checks through clippy-driver rather than rustc, so its artifacts are
# its own and nothing the test build produces stands in for them. The lint
# level goes on the command line, which cargo counts as part of what a unit was
# built with, so leaving it off here would warm artifacts the -D warnings
# verify asks for could not be reused as.
#
# A lint firing is verify's business rather than a reason for a machine to fail
# to set itself up, so a denied warning costs only the artifacts of the crate it
# fired in: --keep-going carries the rest of the workspace on, and the failure
# itself is passed over.
echo 'Warming cargo clippy...'
workspace_selection
if ! cargo clippy --quiet --keep-going "${selection[@]}" --all-targets -- -D warnings; then
    echo 'Clippy has something to say about this checkout; ./scripts/verify will say it.'
fi

echo 'Warming the native test binaries...'
cargo nextest run --cargo-quiet --no-run "${native[@]}"

# Fetches the WASI sysroot, builds the runner, compiles every plugin's tests to
# wasm and leaves a .cwasm beside each of them, which is the whole of what the
# plugin phase does before it starts a test.
echo 'Warming the plugin tests...'
"$internal/test-plugins.sh" --build-only

echo 'Caches are warm.'
