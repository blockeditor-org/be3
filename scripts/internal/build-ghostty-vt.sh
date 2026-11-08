#!/usr/bin/env bash
#
# Builds libghostty-vt for one target, the action crates/be-lib-ghostty-vt's
# :libghostty-vt.a runs. Everything it reads is a pinned download that BUCK
# hands it: zig on PATH, Ghostty's source at a commit, and the tarball of each
# Zig package the build asks for, so nothing here reaches the network.
#
# Usage: build-ghostty-vt.sh --triple TRIPLE --source GHOSTTY.tar.gz
#            --patch PATCH --out ARCHIVE PACKAGE...
#
# GHOSTTY_ZIG_LIBC, when set, is a libc file naming an MSVC runtime and SDK,
# and ANDROID_NDK_HOME the NDK an Android build reads.

set -euo pipefail

triple='' source='' patch='' out=''
packages=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --triple | --source | --patch | --out)
            declare "${1#--}=$2"
            shift 2
            ;;
        *)
            packages+=("$1")
            shift
            ;;
    esac
done
if [[ -z "$triple" || -z "$source" || -z "$patch" || -z "$out" ]]; then
    echo 'Usage: build-ghostty-vt.sh --triple TRIPLE --source GHOSTTY.tar.gz --patch PATCH --out ARCHIVE PACKAGE...' >&2
    exit 1
fi

# libghostty-vt is Zig, and Zig is a cross compiler for every target the app is
# built for, so the same toolchain produces the archive for all of them.
zig_cpu=''
zig_optimize='ReleaseFast'
case "$triple" in
    # The library is only input and output, so the browser gets it as a
    # freestanding archive linked into the app's own module. Freestanding is
    # what also turns off the Kitty graphics protocol, whose image loading
    # wants a filesystem and a clock that WebAssembly has neither of.
    wasm32-*)
        zig_target='wasm32-freestanding'
        # What the app's own objects are compiled with: an object without
        # these cannot be linked into a module with a shared memory at all.
        zig_cpu='generic+atomics+bulk_memory+mutable_globals+sign_ext'
        zig_optimize='ReleaseSmall'
        ;;
    x86_64-unknown-linux-gnu) zig_target='x86_64-linux-gnu' ;;
    aarch64-unknown-linux-gnu) zig_target='aarch64-linux-gnu' ;;
    x86_64-apple-darwin) zig_target='x86_64-macos-none' ;;
    aarch64-apple-darwin) zig_target='aarch64-macos-none' ;;
    aarch64-linux-android) zig_target='aarch64-linux-android' ;;
    x86_64-pc-windows-msvc) zig_target='x86_64-windows-msvc' ;;
    aarch64-pc-windows-msvc) zig_target='aarch64-windows-msvc' ;;
    *)
        echo "No libghostty-vt build is known for target $triple" >&2
        exit 1
        ;;
esac
# Zig names the archive after the target's own convention.
built_name='libghostty-vt.a'
case "$triple" in
    *-windows-*) built_name='ghostty-vt-static.lib' ;;
esac

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT
# Zig keeps packages in a global cache under $HOME, which a worker need not set.
export ZIG_GLOBAL_CACHE_DIR="$scratch/zig-global-cache"

# Applied before building. Ghostty's wuffs package exports weak calloc and free
# stubs whenever libc is not linked into the library, meant only to satisfy a
# freestanding link. Linked into an app against a shared libc, a weak
# definition in the executable still beats the shared library's, so every
# calloc in the app returned null. The patch keeps the stubs to freestanding.
ghostty="$scratch/ghostty"
mkdir -p "$ghostty"
tar -xzf "$source" -C "$ghostty" --strip-components 1
(cd "$ghostty" && patch --quiet -p1) < "$patch"

# Each package into the cache the build reads. The cache is content addressed
# and every package is pinned by hash in a .zon manifest, so the build finds
# them there and fetches nothing; one it still lacks fails the build, naming
# its URL, which is how a newer Ghostty asks for another download in BUCK.
for package in "${packages[@]}"; do
    package="$(realpath "$package")"
    (cd "$ghostty" && zig fetch "$package" > /dev/null)
done

# app-runtime=none and emit-lib-vt leave the terminal emulator and nothing
# else: no GUI, no font stack, no xcframework. SIMD is what would pull in the
# vendored C++ dependencies and a libc, which is more than a debug window
# needs and more than the browser can link.
zig_arguments=(
    build
    -Demit-lib-vt=true
    -Demit-xcframework=false
    -Dapp-runtime=none
    -Dsimd=false
    "-Doptimize=$zig_optimize"
    "-Dtarget=$zig_target"
    --prefix "$scratch/install"
    --cache-dir "$scratch/zig-cache"
)
if [[ -n "$zig_cpu" ]]; then
    zig_arguments+=("-Dcpu=$zig_cpu")
fi
# Zig finds an MSVC installation only on Windows. A build for an MSVC target
# anywhere else names one with a libc file, which is what BUCK does with the
# one xwin lays out.
if [[ -n "${GHOSTTY_ZIG_LIBC:-}" ]]; then
    zig_arguments+=("--libc" "$GHOSTTY_ZIG_LIBC")
fi
(cd "$ghostty" && zig "${zig_arguments[@]}")

cp "$scratch/install/lib/$built_name" "$out"
