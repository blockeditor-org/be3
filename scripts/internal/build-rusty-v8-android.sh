#!/usr/bin/env bash
# Builds the V8 that be-wasm-host runs guests on, for Android (arm64), which
# rusty_v8 publishes no prebuilt of: librusty_v8 and the Rust bindings to it,
# for third-party/v8/BUCK's android_arm64 and android_arm64_binding to pin
# (plans/wasm_on_v8.md says how they are used). A person runs it on their own
# machine, never a worker: it fetches from Chromium's servers and takes about
# an hour on 16 cores and 30 GB of disk.
#
# Needs: Linux on x86_64 (the NDK V8 downloads is linux-x86_64's); git, curl,
# unzip, python3; rustup; Clang 21 or newer with libclang for bindgen
# (LIBCLANG_PATH, e.g. /usr/lib/llvm-21/lib). V8's build downloads the rest:
# NDK r26c, Chromium's clang, gn and ninja, android_platform and catapult.
#
# Usage: scripts/internal/build-rusty-v8-android.sh WORK_DIRECTORY
# It prints the two files to upload and the sha256 and size of each.
set -euo pipefail

# The `v8` crate version crates/be-wasm-host/Cargo.toml pins, and
# third-party/v8/BUCK's _VERSION.
version=152.2.0
target=aarch64-linux-android
work=${1:?usage: $0 WORK_DIRECTORY}

if [ -z "${LIBCLANG_PATH:-}" ]; then
    echo "set LIBCLANG_PATH to Clang 21's lib directory, e.g. /usr/lib/llvm-21/lib" >&2
    exit 1
fi

mkdir -p "$work"
cd "$work"
if [ ! -d rusty_v8 ]; then
    git clone --depth 1 --branch "v$version" --recurse-submodules --shallow-submodules \
        https://github.com/denoland/rusty_v8.git
fi
cd rusty_v8
rustup target add "$target"

# The release profile and default features (use_custom_libcxx, no pointer
# compression or sandbox) are what the other platforms' prebuilts are, and
# what third-party/rust/overlays/v8/build.rs links as.
V8_FROM_SOURCE=1 cargo build --release --lib --target "$target"

gn_out=target/$target/release/gn_out
out=$work/out
mkdir -p "$out"
gzip -9 -c "$gn_out/obj/librusty_v8.a" > "$out/librusty_v8_release_$target.a.gz"
cp "$gn_out/src_binding.rs" "$out/src_binding_release_$target.rs"

echo
echo "Upload these, then put each one's URL, sha256 and size in third-party/v8/BUCK:"
for file in "$out/librusty_v8_release_$target.a.gz" "$out/src_binding_release_$target.rs"; do
    echo "$file"
    echo "  sha256 $(sha256sum "$file" | cut -d' ' -f1)"
    echo "  size_bytes $(stat -c %s "$file")"
done
