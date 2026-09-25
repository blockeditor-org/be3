#!/bin/sh
#
# The part of the NDK a build for aarch64-linux-android reads: the sysroot's
# headers and aarch64 libraries, and clang's Android runtime as a resource
# directory - under 100 of the zip's 780 MB.
#
# Usage: android-ndk.sh URL SHA256 OUT
set -eu
url="$1" sha256="$2" out="$3"
scratch="$(mktemp -d)"
curl --fail --silent --show-error --location --retry 5 --output "$scratch/ndk.zip" "$url"
actual="$(sha256sum "$scratch/ndk.zip" | cut -d ' ' -f 1)"
if [ "$actual" != "$sha256" ]; then
    echo "$url has sha256 $actual, not $sha256" >&2
    exit 1
fi
prebuilt=android-ndk-r29/toolchains/llvm/prebuilt/linux-x86_64
runtime="$prebuilt/lib/clang/21"
unzip -q "$scratch/ndk.zip" -d "$scratch" \
    "$prebuilt/sysroot/usr/include/*" \
    "$prebuilt/sysroot/usr/lib/aarch64-linux-android/*" \
    "$runtime/lib/linux/*aarch64-android*" \
    "$runtime/lib/linux/aarch64/lib*"
mkdir -p "$out/clang"
mv "$scratch/$prebuilt/sysroot" "$out/sysroot"
mv "$scratch/$runtime/lib" "$out/clang/lib"
rm -rf "$scratch"
