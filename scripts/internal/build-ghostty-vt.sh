#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

# The Ghostty commit libghostty-vt is built from. Bump this to pick up a newer
# terminal emulator; the checkout under external/ is refreshed automatically
# when the commit here no longer matches what is in it.
ghostty_repository='https://github.com/ghostty-org/ghostty.git'
ghostty_commit='0c2a290d3a3e2a599be3a43435d778a5896667ee'

# Applied to the checkout before building. Ghostty's wuffs package exports weak
# calloc and free stubs whenever libc is not linked into the library, meant only
# to satisfy a freestanding link. Linked into an app against a shared libc, a
# weak definition in the executable still beats the shared library's, so every
# calloc in the app returned null. The patch keeps the stubs to freestanding.
ghostty_patch="$internal/ghostty-vt.patch"
ghostty_revision="$ghostty_commit $(git hash-object "$ghostty_patch")"

# What Ghostty's build.zig.zon asks for as a minimum, and what CI installs with
# mlugg/setup-zig. A zig on PATH is only used when it is exactly this release,
# since Zig breaks its build API between releases; otherwise this one is
# downloaded into target/tools rather than failing.
zig_version='0.16.0'
# The hashes ziglang.org publishes for that release in its own download index
# (https://ziglang.org/download/index.json), not ones recomputed from these
# downloads. Only the platforms the tarball is fetched for are listed; anywhere
# else ensure_zig asks for a zig on PATH instead of downloading one.
zig_sha256_x86_64_linux='70e49664a74374b48b51e6f3fdfbf437f6395d42509050588bd49abe52ba3d00'
zig_sha256_aarch64_linux='ea4b09bfb22ec6f6c6ceac57ab63efb6b46e17ab08d21f69f3a48b38e1534f17'
zig_sha256_x86_64_macos='0387557ed1877bc6a2e1802c8391953baddba76081876301c522f52977b52ba7'
zig_sha256_aarch64_macos='b23d70deaa879b5c2d486ed3316f7eaa53e84acf6fc9cc747de152450d401489'

triple=''
while [[ $# -gt 0 ]]; do
    case "$1" in
        --triple)
            triple="$2"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

if [[ -z "$triple" ]]; then
    echo 'Usage: build-ghostty-vt.sh --triple TRIPLE' >&2
    exit 1
fi

# Leaves the zig to build with in $zig: the one on PATH if it is the pinned
# release, and otherwise that release, downloaded once into target/tools. Only
# the platforms Zig ships a tarball for are fetched; anywhere else, saying what
# to install is all this can do.
zig='zig'
ensure_zig() {
    if command -v zig > /dev/null && [[ "$(zig version)" == "$zig_version" ]]; then
        return
    fi

    local platform sha256
    case "$(uname -s)-$(uname -m)" in
        Linux-x86_64)
            platform='x86_64-linux'
            sha256="$zig_sha256_x86_64_linux"
            ;;
        Linux-aarch64 | Linux-arm64)
            platform='aarch64-linux'
            sha256="$zig_sha256_aarch64_linux"
            ;;
        Darwin-x86_64)
            platform='x86_64-macos'
            sha256="$zig_sha256_x86_64_macos"
            ;;
        Darwin-arm64)
            platform='aarch64-macos'
            sha256="$zig_sha256_aarch64_macos"
            ;;
        *)
            assert_command zig "Install Zig $zig_version from https://ziglang.org/download."
            echo "Found Zig $(zig version) on PATH, but Ghostty needs Zig $zig_version." >&2
            echo "Install it from https://ziglang.org/download." >&2
            exit 1
            ;;
    esac

    local name="zig-$platform-$zig_version"
    local tools="$repository/target/tools"
    local directory="$tools/$name"
    zig="$directory/zig"
    if [[ -x "$zig" ]]; then
        return
    fi

    assert_command curl 'Install curl, or install Zig yourself from https://ziglang.org/download.'
    assert_command tar 'Install tar, or install Zig yourself from https://ziglang.org/download.'
    local url="https://ziglang.org/download/$zig_version/$name.tar.xz"
    local archive="$tools/$name.tar.xz"
    echo "Downloading Zig $zig_version from $url..." >&2
    mkdir -p "$tools"
    # The extraction is into a directory named after the release, so an
    # interrupted one leaves nothing that a later run would take for complete.
    rm -rf "$directory" "$archive"
    download_verified "$url" "$archive" "$sha256"
    tar -xJf "$archive" -C "$tools"
    rm "$archive"
    if [[ ! -x "$zig" ]]; then
        echo "The Zig tarball did not contain $zig" >&2
        exit 1
    fi
}

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
    x86_64-unknown-linux-musl) zig_target='x86_64-linux-musl' ;;
    aarch64-unknown-linux-musl) zig_target='aarch64-linux-musl' ;;
    x86_64-apple-darwin) zig_target='x86_64-macos-none' ;;
    aarch64-apple-darwin) zig_target='aarch64-macos-none' ;;
    aarch64-linux-android) zig_target='aarch64-linux-android' ;;
    x86_64-linux-android) zig_target='x86_64-linux-android' ;;
    armv7-linux-androideabi) zig_target='arm-linux-androideabi' ;;
    x86_64-pc-windows-msvc) zig_target='x86_64-windows-msvc' ;;
    aarch64-pc-windows-msvc) zig_target='aarch64-windows-msvc' ;;
    x86_64-pc-windows-gnu) zig_target='x86_64-windows-gnu' ;;
    aarch64-pc-windows-gnullvm) zig_target='aarch64-windows-gnu' ;;
    *)
        echo "No libghostty-vt build is known for target $triple" >&2
        exit 1
        ;;
esac

output="$repository/target/ghostty-vt/$triple"
# Zig names the archive after the target's own convention, and the MSVC linker
# wants that name; the GNU toolchains want the lib prefix that -l resolves.
built_name='libghostty-vt.a'
archive_name='libghostty-vt.a'
case "$triple" in
    *-windows-msvc)
        built_name='ghostty-vt-static.lib'
        archive_name='ghostty-vt-static.lib'
        ;;
    *-windows-*) built_name='ghostty-vt-static.lib' ;;
esac
archive="$output/$archive_name"

# Everything that decides what ends up in the archive, so that changing any of
# it rebuilds rather than leaving a stale artifact behind.
stamp="$output/.stamp"
stamp_contents="$ghostty_revision $zig_target $zig_cpu $zig_optimize"
if [[ -f "$archive" && -f "$stamp" && "$(cat "$stamp")" == "$stamp_contents" ]]; then
    echo "$archive"
    exit 0
fi

assert_command git 'Install git.'
ensure_zig

# The checkout is deliberately outside the target directory: it is well over a
# hundred megabytes and survives a cargo clean, and only ever holds the one
# commit that is fetched into it.
source_directory="$repository/external/ghostty"
if [[ "$(cat "$source_directory/.commit" 2> /dev/null)" != "$ghostty_revision" ]]; then
    echo "Fetching Ghostty $ghostty_commit..." >&2
    rm -rf "$source_directory"
    mkdir -p "$source_directory"
    git -C "$source_directory" init --quiet
    git -C "$source_directory" remote add origin "$ghostty_repository"
    git -C "$source_directory" fetch --quiet --depth 1 origin "$ghostty_commit"
    git -C "$source_directory" checkout --quiet FETCH_HEAD
    git -C "$source_directory" apply "$ghostty_patch"
    echo "$ghostty_revision" > "$source_directory/.commit"
fi

install_prefix="$output/install"
rm -rf "$install_prefix" "$archive" "$stamp"
mkdir -p "$output"

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
)
if [[ -n "$zig_cpu" ]]; then
    zig_arguments+=("-Dcpu=$zig_cpu")
fi

build_libghostty() {
    (
        cd "$source_directory"
        "$zig" "${zig_arguments[@]}" \
            --prefix "$install_prefix" \
            --cache-dir "$repository/target/ghostty-vt/zig-cache"
    )
}

# Checks a git dependency out at the commit it is pinned to, without its .git,
# which is the same tree zig would have produced and hashes to the same package
# name. Asking the remote for the one commit is what works for a commit that no
# branch points at any more; a server that refuses that is cloned whole instead.
prefetch_git() {
    local remote="$1" commit="$2" directory="$3"
    rm -rf "$directory.tmp"
    mkdir -p "$directory.tmp"
    git -C "$directory.tmp" init --quiet
    git -C "$directory.tmp" remote add origin "$remote"
    if [[ -n "$commit" ]] \
        && git -C "$directory.tmp" fetch --quiet --depth 1 origin "$commit" 2> /dev/null; then
        git -C "$directory.tmp" checkout --quiet FETCH_HEAD
    else
        rm -rf "$directory.tmp"
        # Blobless, because this is the fallback for a server that would not
        # hand over the one commit, and a manifest naming something that no
        # longer exists should cost a listing rather than a whole repository.
        git clone --quiet --filter=blob:none "$remote" "$directory.tmp" 2> /dev/null || return 1
        if [[ -n "$commit" ]]; then
            git -C "$directory.tmp" checkout --quiet "$commit" 2> /dev/null || return 1
        fi
    fi
    rm -rf "$directory.tmp/.git"
    mv "$directory.tmp" "$directory"
}

# Hands `zig fetch` one dependency that was downloaded with something else, so
# that it lands in the package cache the build reads and is never fetched over
# the network again. A tarball keeps its extension, because that is how zig
# recognises the compression. A GitHub tarball that will not download is
# checked out instead: some networks serve the git endpoints and not the
# archive ones, and a repository's archive of a commit holds what a checkout of
# it does.
prefetch_url() {
    local url="$1" name path remote commit
    name="$(printf '%s' "$url" | git hash-object --stdin)"
    case "$url" in
        git+*)
            remote="${url#git+}"
            commit=''
            case "$remote" in
                *'#'*)
                    commit="${remote##*#}"
                    remote="${remote%%#*}"
                    ;;
            esac
            remote="${remote%%\?*}"
            path="$prefetch_directory/$name"
            [[ -d "$path" ]] || prefetch_git "$remote" "$commit" "$path" || return 1
            ;;
        *)
            path="$prefetch_directory/$name-$(basename "$url")"
            if [[ ! -s "$path" ]] \
                && ! curl --fail --silent --location --output "$path" "$url"; then
                rm -f "$path"
                case "$url" in
                    https://github.com/*/archive/*)
                        remote="${url%/archive/*}"
                        commit="$(basename "$url")"
                        commit="${commit%%.tar*}"
                        commit="${commit%%.zip}"
                        path="$prefetch_directory/$name"
                        [[ -d "$path" ]] || prefetch_git "$remote" "$commit" "$path" || return 1
                        ;;
                    *) return 1 ;;
                esac
            fi
            ;;
    esac
    (cd "$source_directory" && "$zig" fetch "$path" > /dev/null)
}

# On a machine whose egress is a proxy - a CI sandbox, a corporate network -
# Zig's package fetcher cannot download anything, so the build never starts.
# As of 0.16 it does read HTTPS_PROXY and issues a correct CONNECT, but having
# been told the tunnel is open it waits to read instead of starting the TLS
# handshake, and the proxy closes an idle tunnel. So this is not something a
# newer Zig has fixed; it was retested on 0.16 and still hangs.
#
# curl and git go through a proxy properly, so when the build fails this
# fetches what the manifests name with those instead and hands each one to
# `zig fetch`. That is only a transport: the package cache is content
# addressed, and every dependency is pinned by hash in a .zon manifest, so a
# tarball that arrived altered hashes to a name the build is not looking for
# and fails rather than being used. Unpacking a package reveals the manifest of
# its own dependencies, so this repeats until a pass turns up nothing new. A
# URL that cannot be fetched is left to the build to complain about: several of
# them are optional packages for platforms this library is not built for, and
# one is a placeholder in Ghostty's own documentation.
#
# The proxy is what the sandbox enforces its egress policy with, so this works
# within it rather than around it: nothing here unsets HTTPS_PROXY.
prefetch_dependencies() {
    echo 'Fetching Ghostty dependencies without zig, for a proxied network...' >&2
    mkdir -p "$prefetch_directory"
    local seen=' ' url pass found
    for pass in 1 2 3 4 5; do
        found=0
        while read -r url; do
            [[ -z "$url" || "$seen" == *" $url "* ]] && continue
            seen+="$url "
            found=1
            prefetch_url "$url" || true
        done < <(grep -rho '\.url = "[^"]*"' "$source_directory" --include='*.zon' \
            | sed -e 's/^\.url = "//' -e 's/"$//' | sort -u)
        if [[ "$found" -eq 0 ]]; then
            break
        fi
    done
}

prefetch_directory="$repository/target/ghostty-vt/prefetch"

echo "Building libghostty-vt for $triple ($zig_target)..." >&2
if ! build_libghostty; then
    prefetch_dependencies
    build_libghostty
fi

built="$install_prefix/lib/$built_name"
if [[ ! -f "$built" ]]; then
    echo "zig did not produce $built" >&2
    exit 1
fi
mv "$built" "$archive"
rm -rf "$install_prefix"
echo "$stamp_contents" > "$stamp"
echo "$archive"
