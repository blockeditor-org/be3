#!/bin/sh
#
# deb_sysroot's action (defs.bzl): the packages unpacked and laid out as a
# sysroot in OUT. --keep-xkb also keeps the XKB keymaps.
#
# Usage: deb_sysroot.sh OUT [--keep-xkb] PACKAGE...
set -eu
out="$1"
shift
xkb=''
if [ "${1:-}" = --keep-xkb ]; then
    xkb=usr/share/X11/xkb
    shift
fi
unpacked="$(mktemp -d)"
for package; do dpkg-deb -x "$package" "$unpacked"; done
for directory in lib lib64; do
    if [ -d "$unpacked/$directory" ] && [ ! -L "$unpacked/$directory" ]; then
        mkdir -p "$unpacked/usr/$directory"
        cp -a "$unpacked/$directory/." "$unpacked/usr/$directory/"
    fi
done
mkdir -p "$out/usr/lib"
for kept in usr/include usr/lib64 usr/lib/gcc usr/lib/pkgconfig usr/share/pkgconfig \
    usr/lib/x86_64-linux-gnu usr/lib/aarch64-linux-gnu $xkb; do
    if [ -d "$unpacked/$kept" ]; then
        mkdir -p "$out/$(dirname "$kept")"
        cp -a "$unpacked/$kept" "$out/$kept"
    fi
done
# The dynamic loader is directly in usr/lib on arm64, where glibc's libc.so
# linker script looks for it; amd64's is in usr/lib64, which is kept whole.
for loader in "$unpacked"/usr/lib/ld-linux-*.so*; do
    if [ -e "$loader" ]; then cp -a "$loader" "$out/usr/lib/"; fi
done
rm -rf "$unpacked"
cd "$out"
ln -s usr/lib lib
if [ -d usr/lib64 ]; then ln -s usr/lib64 lib64; fi
find . -type l | while IFS= read -r link; do
    target="$(readlink "$link")"
    case "$target" in
        /*)
            directory="$(dirname "$link")"
            up="$(printf '%s' "${directory#./}" | sed -e 's|[^/][^/]*|..|g')"
            ln -sfn "$up${target}" "$link"
            ;;
    esac
done
find . -xtype l -delete
find . -name '*\*' -exec rm -rf {} +
