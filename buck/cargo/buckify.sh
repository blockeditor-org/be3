#!/bin/sh
#
# buckify.bxl's action: cargo's plans for every build buck2 stands in for, and
# crates.bzl generated from them by buck-tools, in OUT beside the Cargo.lock
# they were planned with.
#
# Usage: buckify.sh OUT WORKSPACE PLACEHOLDERS TOOLCHAIN NIGHTLY [PLAN=TRIPLE]...
#   each PLAN=TRIPLE a cross-compiled platform (buck/platforms/cross.bzl)
set -eu
out="$(pwd)/$1"
workspace="$2"
layout="$(pwd)/$3"
toolchain="$(cd "$4" && pwd)"
nightly="$(cd "$5" && pwd)"
shift 5
scratch="$(mktemp -d)"
mkdir -p "$out"
cp -RL "$workspace/." "$scratch/workspace"
cd "$scratch/workspace"
while IFS= read -r path || [ -n "$path" ]; do
    mkdir -p "$(dirname "$path")"
    : > "$path"
done < "$layout"
for manifest in $(find crates -name Cargo.toml); do
    sed -n 's/^[[:space:]]*path[[:space:]]*=[[:space:]]*"\([^"]*\.rs\)".*/\1/p' "$manifest" | while IFS= read -r relative; do
        path="$(dirname "$manifest")/$relative"
        mkdir -p "$(dirname "$path")"
        [ -e "$path" ] || : > "$path"
    done
done
export CARGO_HOME="$scratch/cargo-home"

# Cargo.lock as the cargo a developer runs would leave it: brought up to date
# with the manifests, changing nothing it does not have to. Everything below
# reads it, and //:verify's lint writes it back to the checkout.
PATH="$toolchain/bin:$PATH" "$toolchain/bin/cargo" metadata --format-version 1 --quiet > /dev/null
cp Cargo.lock "$out/Cargo.lock"

# Every crate archive's size beside its hash, so that buck2 knows each digest
# without asking crates.io for it on every new daemon. cargo fetch downloads
# every crate Cargo.lock names, for every platform.
PATH="$toolchain/bin:$PATH" "$toolchain/bin/cargo" fetch --locked --quiet
for crate in "$CARGO_HOME"/registry/cache/*/*.crate; do
    printf '%s %s\n' "$(basename "$crate" .crate)" "$(wc -c < "$crate")"
done > "$scratch/sizes"

plugins=""
excluded=""
package_name() {
    sed -n 's/^name[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' "$1" | head -n 1
}
for manifest in crates/editors/*/Cargo.toml; do
    name="$(package_name "$manifest")"
    plugins="$plugins -p $name"
    excluded="$excluded --exclude $name"
done
games=""
for manifest in crates/tabletop-games/rules/*/Cargo.toml; do
    games="$games -p $(package_name "$manifest")"
done

# The builds buck2 stands in for, as cargo would be asked for them: the host
# (block-app with full, without the wasm-only crates), block-app and beui's
# DOM demo for the web, the plugins and beui's demo, whose tests paint as a
# plugin's do (a plan of their own: they want a different wgpu), the games and
# the gpu shim, and the host again for each cross target. `cargo test` where
# there are tests, so the plan has the dev-dependencies. Every plan names its
# target, so what cargo builds for the machine running the build - proc
# macros, build scripts and what they depend on - is told apart from the rest.
# --unit-graph needs the nightly.
plan() {
    RUSTC="$nightly/bin/rustc" "$nightly/bin/cargo" "$@" --unit-graph -Z unstable-options --locked
}
RUSTC="$nightly/bin/rustc" "$nightly/bin/cargo" metadata --all-features --format-version 1 --locked > "$scratch/metadata.json"
host_packages="--workspace $excluded --exclude block-editor-plugin --exclude block-editor-beui --exclude block-gpu-shim --exclude beui-web-demo"
plan test --target x86_64-unknown-linux-gnu $host_packages --features block-app/full > "$scratch/host.json"
plan build --target wasm32-wasip1-threads -p block-app -p beui-web-demo --lib --features block-app/full > "$scratch/wasi.json"
plan test --target wasm32-wasip1-threads $plugins -p block-editor-plugin -p block-editor-beui -p beui-demo > "$scratch/wasi-guest.json"
plan build --target wasm32-unknown-unknown $games -p block-gpu-shim > "$scratch/wasm32.json"
cross=""
for platform in "$@"; do
    name="${platform%%=*}"
    plan test --target "${platform#*=}" $host_packages --features block-app/full > "$scratch/$name.json"
    cross="$cross $name=$scratch/$name.json"
done
CARGO_TARGET_DIR="$scratch/target" PATH="$toolchain/bin:$PATH" \
    "$toolchain/bin/cargo" build --quiet --release --locked -p buck-tools
"$scratch/target/release/buck-tools" generate "$scratch/metadata.json" Cargo.lock "$scratch/sizes" Cargo.toml \
    "linux-x86_64=$scratch/host.json" "wasi=$scratch/wasi.json" "wasi-guest=$scratch/wasi-guest.json" "wasm32=$scratch/wasm32.json" \
    $cross \
    > "$out/crates.bzl"
rm -rf "$scratch"
