#!/usr/bin/env bash
# Builds the V8 that be-wasm-host runs guests on, for Android (arm64), which
# rusty_v8 publishes no prebuilt of: librusty_v8 and the Rust bindings to it,
# for third-party/v8/BUCK's android_arm64 and android_arm64_binding to pin.
# A person runs it on their own
# machine, never a worker: it fetches from Chromium's servers and takes about
# an hour on 16 cores and 30 GB of disk.
#
# Needs: Linux on x86_64 (the NDK V8 downloads is linux-x86_64's); git, curl,
# unzip, python3; Rust with the aarch64-linux-android target (rustup adds it
# when present); Clang 21 or newer with libclang for bindgen
# (LIBCLANG_PATH, e.g. /usr/lib/llvm-21/lib). V8's build downloads the rest:
# NDK r26c, Chromium's clang, gn and ninja, android_platform and catapult. With
# LIBCLANG_PATH set, V8 may compile with that Clang rather than Chromium's,
# which the other platforms' prebuilts were built with; either works.
#
# rusty_v8 does not build Android in its CI, and at v152.2.0 its build needs
# three fixes, made below: its fork of Chromium's build/ never sets
# android_ndk_version and looks for the NDK where build.rs does not put it;
# V8's host tools need Chromium's Debian sysroot; and build.rs gives bindgen no
# sysroot for Android, so it would read the host's glibc headers. The 152.2.0
# files pinned were built with these fixes made by hand; a clean run of this
# script with them folded in has not been confirmed yet.
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
if command -v rustup > /dev/null; then
    rustup target add "$target"
fi

# The NDK is the r26c build.rs downloads to third_party/android_ndk.
ndk=$PWD/third_party/android_ndk
config=build/config/android/config.gni
if ! grep -q 'android_ndk_version = "r26c"' "$config"; then
    sed -i 's|^  android_ndk_root = "//third_party/android_toolchain/ndk"$|  android_ndk_root = "//third_party/android_ndk"\n  android_ndk_version = "r26c"|' "$config"
    grep -q 'android_ndk_version = "r26c"' "$config" || {
        echo "$config no longer has the android_ndk_root line this patches" >&2
        exit 1
    }
fi
python3 build/linux/sysroot_scripts/install-sysroot.py --arch=amd64
export BINDGEN_EXTRA_CLANG_ARGS_aarch64_linux_android="--sysroot=$ndk/toolchains/llvm/prebuilt/linux-x86_64/sysroot"

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
