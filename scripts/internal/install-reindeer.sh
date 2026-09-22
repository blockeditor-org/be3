#!/usr/bin/env bash
#
# Puts the pinned reindeer in cargo's bin directory.
#
# reindeer is what turns the workspace's Cargo.toml into third-party/rust/BUCK,
# and ./scripts/buckify is the only thing that runs it. A build does not need
# it: the file it writes is checked in.
#
# It publishes no releases at all, so there is nothing to download and it is
# built from the commit common.sh pins. It is a small program; this takes about
# a minute, not the quarter of an hour buck2 does.
#
# Usage:
#   install-reindeer.sh

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

if [[ $# -ne 0 ]]; then
    echo 'Usage: install-reindeer.sh' >&2
    exit 1
fi

if command -v reindeer > /dev/null 2>&1; then
    echo 'reindeer is already installed.'
    exit 0
fi

assert_command cargo 'Install Rust from https://rustup.rs.'
assert_command rustup 'Install Rust from https://rustup.rs.'
time_script 'Installing reindeer'

step "Installing the $buck2_toolchain toolchain reindeer is built with"
rustup toolchain install "$buck2_toolchain" --profile minimal

step "Building reindeer from $reindeer_revision"
cargo "+$buck2_toolchain" install \
    --git https://github.com/facebookincubator/reindeer.git \
    --rev "$reindeer_revision" \
    --locked \
    reindeer
end_step

echo 'Installed reindeer.'
