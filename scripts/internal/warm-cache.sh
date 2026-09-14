#!/usr/bin/env bash
#
# Compiles everything ./scripts/verify needs, without running a test.
#
# Verify is only slow the first time. On a machine whose target directory is
# already populated it finishes in well under a minute; on a fresh checkout it
# spends a quarter of an hour compiling before the first test runs, because
# every phase of it wants a different set of artifacts: clippy wants the
# workspace checked with every feature on, the test phase wants it built for
# real, and the plugin phase wants a WASI sysroot, an optimised runner and
# every plugin's tests compiled to wasm and handed to Cranelift.
#
# This builds all of that and stops there. ./scripts/setup runs it so the
# waiting happens once, while the machine is being prepared, rather than in the
# middle of the first verify. Running it again later is cheap: it is the same
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
load_plugins
native=(--workspace)
for plugin in "${plugins[@]}"; do
    native+=(--exclude "$plugin")
done

echo 'Warming libghostty-vt...'
host_triple="$(rustc --version --verbose | sed -n 's/^host: //p')"
"$internal/build-ghostty-vt.sh" --triple "$host_triple" > /dev/null

echo 'Warming crates/fix-rust-source...'
cargo build --quiet -p fix-rust-source

# Clippy checks through clippy-driver rather than rustc, so its artifacts are
# its own: none of what the test build produces stands in for them, and the
# warning cap is left alone here because a lint firing is verify's business,
# not a reason for a machine to fail to set itself up.
echo 'Warming cargo clippy...'
cargo clippy --quiet --workspace --all-targets --all-features

echo 'Warming the native test binaries...'
cargo nextest run --cargo-quiet --no-run "${native[@]}"

# Fetches the WASI sysroot, builds the runner, compiles every plugin's tests to
# wasm and leaves a .cwasm beside each of them, which is the whole of what the
# plugin phase does before it starts a test.
echo 'Warming the plugin tests...'
"$internal/test-plugins.sh" --build-only

echo 'Caches are warm.'
