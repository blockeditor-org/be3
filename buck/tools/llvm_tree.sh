#!/bin/sh
#
# llvm_tree's action (defs.bzl): Ubuntu's clang packages unpacked into a
# self-contained llvm-20 tree in OUT.
#
# Usage: llvm_tree.sh OUT PACKAGE...
set -eu
out="$1"
shift
packages="$(mktemp -d)"
for package; do dpkg-deb -x "$package" "$packages"; done
mkdir -p "$out/bin" "$out/lib"
cp -P "$packages"/usr/lib/llvm-20/bin/clang "$packages"/usr/lib/llvm-20/bin/clang++ \
    "$packages"/usr/lib/llvm-20/bin/lld "$packages"/usr/lib/llvm-20/bin/ld.lld \
    "$packages"/usr/lib/llvm-20/bin/ld64.lld "$packages"/usr/lib/llvm-20/bin/lld-link \
    "$packages"/usr/lib/llvm-20/bin/llvm-ar "$packages"/usr/lib/llvm-20/bin/llvm-lib "$out/bin/"
cp "$packages"/usr/lib/x86_64-linux-gnu/libLLVM.so.20.1 \
    "$packages"/usr/lib/x86_64-linux-gnu/libclang-cpp.so.20.1 "$out/lib/"
cp "$packages"/usr/lib/x86_64-linux-gnu/libclang-20.so.20 "$out/lib/libclang.so"
cp -R "$packages"/usr/lib/llvm-20/lib/clang "$out/lib/"
rm -rf "$packages"
