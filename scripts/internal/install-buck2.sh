#!/usr/bin/env bash
#
# Puts the pinned buck2 in cargo's bin directory.
#
# It is built from the commit common.sh pins rather than downloaded, because
# buck2 ships its binaries as GitHub release assets and there is no hash
# published beside them to pin the bytes against; everything else this
# repository downloads is checked against one. Building from a commit is the
# same on every machine, and the commit pins the prelude too, since a buck2
# binary carries the build rules it was built with.
#
# It is not cheap: a cold build is around fifteen minutes. CI keeps its cargo
# directory between runs, so it pays that once per pin rather than once a run.
#
# A buck2 already on PATH is left alone, whatever version it is, the same way
# install-nextest.sh leaves a cargo-nextest alone.
#
# Usage:
#   install-buck2.sh

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

if [[ $# -ne 0 ]]; then
    echo 'Usage: install-buck2.sh' >&2
    exit 1
fi

if command -v buck2 > /dev/null 2>&1; then
    echo "buck2 is already installed: $(buck2 --version)"
    exit 0
fi

assert_command cargo 'Install Rust from https://rustup.rs.'
assert_command rustup 'Install Rust from https://rustup.rs.'
time_script 'Installing buck2'

step "Installing the $buck2_toolchain toolchain buck2 is built with"
rustup toolchain install "$buck2_toolchain" --profile minimal \
    --component llvm-tools-preview --component rust-src

step "Building buck2 from $buck2_revision"
cargo "+$buck2_toolchain" install \
    --git https://github.com/facebook/buck2.git \
    --rev "$buck2_revision" \
    --locked \
    buck2
end_step

echo "Installed $(buck2 --version)"
