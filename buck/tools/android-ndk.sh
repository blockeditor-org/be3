#!/bin/sh
#
# The part of the NDK a build for aarch64-linux-android reads: the sysroot's
# headers and aarch64 libraries, and clang's Android runtime as a resource
# directory - under 100 of the zip's 780 MB.
#
# Usage: android-ndk.sh ZIP OUT
set -eu
zip="$1" out="$2"
scratch="$(mktemp -d)"
prebuilt=android-ndk-r29/toolchains/llvm/prebuilt/linux-x86_64
runtime="$prebuilt/lib/clang/21"
unzip -q "$zip" -d "$scratch" \
    "$prebuilt/sysroot/usr/include/*" \
    "$prebuilt/sysroot/usr/lib/aarch64-linux-android/*" \
    "$runtime/lib/linux/*aarch64-android*" \
    "$runtime/lib/linux/aarch64/lib*"
mkdir -p "$out/clang"
mv "$scratch/$prebuilt/sysroot" "$out/sysroot"
mv "$scratch/$runtime/lib" "$out/clang/lib"
rm -rf "$scratch"
