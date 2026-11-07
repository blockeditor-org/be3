#!/bin/sh
#
# buckify.bxl's action: cargo's plans for every build buck2 stands in for, and
# crates.bzl generated from them by buck-tools, in OUT beside the Cargo.lock
# they were planned with.
#
# Usage: buckify.sh OUT WORKSPACE PLACEHOLDERS TOOLCHAIN NIGHTLY [PLAN=TRIPLE]... -- [CRATE]...
#   each PLAN=TRIPLE a cross-compiled platform (buck/platforms/cross.bzl), and
#   each CRATE KEY=SHA256=SIZE=DIRECTORY, a crate the checked-in crates.bzl
#   downloads, unpacked
set -eu
out="$(pwd)/$1"
workspace="$2"
layout="$(pwd)/$3"
toolchain="$(cd "$4" && pwd)"
nightly="$(cd "$5" && pwd)"
shift 5
scratch="$(mktemp -d)"
cross_platforms=""
while [ "$1" != "--" ]; do
    cross_platforms="$cross_platforms $1"
    shift
done
shift

# The crates as a cargo directory source: each crate's files, linked, beside
# the checksum Cargo.lock has for it, and their sizes from crates.bzl.
vendor="$scratch/vendor"
mkdir -p "$vendor"
: > "$scratch/vendored-sizes"
for crate in "$@"; do
    key="${crate%%=*}"
    rest="${crate#*=}"
    sha256="${rest%%=*}"
    rest="${rest#*=}"
    size="${rest%%=*}"
    directory="$(pwd)/${rest#*=}"
    mkdir "$vendor/$key"
    for file in "$directory"/* "$directory"/.[!.]*; do
        if [ -e "$file" ]; then
            ln -s "$file" "$vendor/$key/"
        fi
    done
    printf '{"files":{},"package":"%s"}' "$sha256" > "$vendor/$key/.cargo-checksum.json"
    printf '%s %s\n' "$key" "$size" >> "$scratch/vendored-sizes"
done
mkdir -p "$out"
cp -RL "$workspace/." "$scratch/workspace"
# buck2 hands the action its inputs read-only, and cp keeps their modes, but
# cargo rewrites Cargo.lock in place when the manifests have changed.
chmod -R u+w "$scratch/workspace"
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
mkdir -p "$CARGO_HOME"

# With Cargo.lock up to date with the manifests and every crate it names
# vendored, cargo reads the vendored crates in place of crates.io and needs no
# network: buck2 has them already, from builds. A lockfile that needs anything
# else, such as a new dependency, is resolved against crates.io instead.
printf '[source.crates-io]\nreplace-with = "vendored"\n\n[source.vendored]\ndirectory = "%s"\n' "$vendor" > "$CARGO_HOME/config.toml"
if PATH="$toolchain/bin:$PATH" "$toolchain/bin/cargo" metadata --format-version 1 --quiet --locked > /dev/null 2>&1; then
    cp "$scratch/vendored-sizes" "$scratch/sizes"
else
    echo "buckify: Cargo.lock needs crates that buck/cargo/crates.bzl does not download yet, so cargo downloads every crate it names" >&2
    rm "$CARGO_HOME/config.toml"
    # Cargo.lock as the cargo a developer runs would leave it: brought up to
    # date with the manifests, changing nothing it does not have to.
    PATH="$toolchain/bin:$PATH" "$toolchain/bin/cargo" metadata --format-version 1 --quiet > /dev/null
    # Every crate archive's size beside its hash, so that buck2 knows each
    # digest without asking crates.io for it on every new daemon. cargo fetch
    # downloads every crate Cargo.lock names, for every platform.
    PATH="$toolchain/bin:$PATH" "$toolchain/bin/cargo" fetch --locked --quiet
    for crate in "$CARGO_HOME"/registry/cache/*/*.crate; do
        printf '%s %s\n' "$(basename "$crate" .crate)" "$(wc -c < "$crate")"
    done > "$scratch/sizes"
fi
# Everything below reads Cargo.lock, and ./scripts/verify's lint writes it
# back to the checkout.
cp Cargo.lock "$out/Cargo.lock"

apps=""
excluded=""
package_name() {
    sed -n 's/^name[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' "$1" | head -n 1
}
# An editor with examples is built for the host as well, where they run
# (editor() in buck/wasm/defs.bzl).
for manifest in crates/be-app-*/Cargo.toml; do
    name="$(package_name "$manifest")"
    apps="$apps -p $name"
    [ -d "$(dirname "$manifest")/examples" ] || excluded="$excluded --exclude $name"
done
games=""
for manifest in crates/tabletop-rules-*/Cargo.toml; do
    games="$games -p $(package_name "$manifest")"
done
# The libraries apps are linked against and the crates that test linking
# them (wasm_library and linked_app in buck/wasm/defs.bzl), which are guests.
libraries=""
for manifest in crates/be-sdk-library-*/Cargo.toml crates/be-tool-tls-diamond-*/Cargo.toml; do
    libraries="$libraries -p $(package_name "$manifest")"
done
types=""
# Every app's type module; be-type-bundle runs them in process on the host.
for manifest in crates/be-type-*/Cargo.toml; do
    [ "$manifest" = crates/be-type-bundle/Cargo.toml ] && continue
    types="$types -p $(package_name "$manifest")"
done

# The builds buck2 stands in for, as cargo would be asked for them: the host
# (without the wasm-only crates), be-host, beui's DOM
# demo, the paint review page and beui-html's live DOM for the web, the apps, beui's demo and the
# paint review, whose tests paint as an app's do (a plan of their own: they
# want a different wgpu), the games, the gpu shim, the remap module and the
# pure guests (the apps' type modules among them), and
# the host again for each cross target. `cargo test` where
# there are tests, so the plan has the dev-dependencies. Every plan names its
# target, so what cargo builds for the machine running the build - proc
# macros, build scripts and what they depend on - is told apart from the rest.
# --unit-graph needs the nightly.
plan() {
    RUSTC="$nightly/bin/rustc" "$nightly/bin/cargo" "$@" --unit-graph -Z unstable-options --locked
}
RUSTC="$nightly/bin/rustc" "$nightly/bin/cargo" metadata --all-features --format-version 1 --locked > "$scratch/metadata.json"
host_packages="--workspace $excluded --exclude be-sdk --exclude be-sdk-beui --exclude be-wasm-gpu-shim --exclude beui-web-demo --exclude beui-html-spike --exclude be-server-portal-ui --exclude be-server-bin --exclude beui-demo-html --exclude be-tool-paint-review-web"
plan test --target x86_64-unknown-linux-gnu $host_packages > "$scratch/host.json"
plan build --target wasm32-wasip1-threads -p be-host -p beui-web-demo -p be-tool-paint-review-web -p beui-html --lib > "$scratch/wasi.json"
plan test --target wasm32-wasip1-threads $apps $libraries -p be-sdk -p be-sdk-beui -p beui-demo -p be-tool-paint-review > "$scratch/wasi-guest.json"
plan build --target wasm32-unknown-unknown $games $types -p be-wasm-gpu-shim -p be-host-remap-module -p be-wasm-guest-echo > "$scratch/wasm32.json"
# beui's HTML core (buck/constraints:html): the crates that build on it, for
# the browser and for the host, under the cfg that picks it. cargo evaluates
# [target.'cfg(beui_html)'.dependencies] against RUSTFLAGS, so each core's
# dependencies drop out of the other's plans.
# The browser's plan adds the crates whose native side runs only on the
# native core, such as the shared tests' fixtures program, and beui's demo
# in the browser (beui-demo-html). The host's adds be-server-bin, the
# server's executable, which renders its account portal on that core.
html_packages="-p beui -p beui-dock -p beui-dock-host -p beui-dock-state -p beui-keyboard -p beui-view -p beui-html -p beui-core-html -p beui-components-unstyled -p beui-components-styled -p be-host-accounts-ui -p beui-html-spike -p be-server-portal-ui"
browser_packages="$html_packages -p beui-tests -p beui-demo-html"
host_html_packages="$html_packages -p be-server-bin"
RUSTFLAGS="--cfg beui_html" plan build --target wasm32-wasip1-threads $browser_packages --lib > "$scratch/wasi-html.json"
RUSTFLAGS="--cfg beui_html" plan test --target x86_64-unknown-linux-gnu $host_html_packages > "$scratch/linux-x86_64-html.json"
cross=""
for platform in $cross_platforms; do
    name="${platform%%=*}"
    plan test --target "${platform#*=}" $host_packages > "$scratch/$name.json"
    cross="$cross $name=$scratch/$name.json"
done
CARGO_TARGET_DIR="$scratch/target" PATH="$toolchain/bin:$PATH" \
    "$toolchain/bin/cargo" build --quiet --release --locked -p be-tool-buck
"$scratch/target/release/buck-tools" generate "$scratch/metadata.json" Cargo.lock "$scratch/sizes" Cargo.toml \
    "linux-x86_64=$scratch/host.json" "wasi=$scratch/wasi.json" "wasi-guest=$scratch/wasi-guest.json" "wasm32=$scratch/wasm32.json" \
    "wasi-html=$scratch/wasi-html.json" "linux-x86_64-html=$scratch/linux-x86_64-html.json" \
    $cross \
    > "$out/crates.bzl"
rm -rf "$scratch"
