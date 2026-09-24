# The rules behind buck/sysroot/BUCK.

# Resolves the package closure on a worker, which fetches the snapshot's
# package indices itself. ./scripts/buckify copies what this writes to
# buck/sysroot/packages.bzl. The snapshot and the packages are the whole input,
# so the answer is cached until one of them changes.
def _deb_lock_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output("packages.bzl")
    ctx.actions.run(
        cmd_args(
            "sh",
            "-c",
            'out="$1"; shift; python3 "$@" > "$out"',
            "--",
            out.as_output(),
            ctx.attrs.resolver,
            ctx.attrs.snapshot,
            ["{}:{}:{}".format(name, architecture, ",".join(packages)) for name, (architecture, packages) in ctx.attrs.sets.items()],
        ),
        category = "deb_lock",
    )
    return [DefaultInfo(default_output = out)]

deb_lock = rule(
    attrs = {
        "resolver": attrs.source(),
        "sets": attrs.dict(attrs.string(), attrs.tuple(attrs.string(), attrs.list(attrs.string()))),
        "snapshot": attrs.string(),
    },
    impl = _deb_lock_impl,
)

# The packages laid out as a sysroot: what a compiler, a linker and a program
# run against it read, and nothing else. Each .deb is unpacked into one scratch
# tree, and what is kept is the headers, the libraries - with the loadable
# modules GTK and WebKitGTK keep beside them - the gcc install clang takes the
# C++ runtime from, and the pkg-config files. Everything else a package ships,
# systemd units and translations and binaries, stays behind.
#
# Two things make it usable from anywhere. Ubuntu 24.04 has merged /usr, so
# /lib and /lib64 are links into usr/ the way they are on the real root. And a
# package's absolute symlinks - libfoo.so pointing at
# /usr/lib/x86_64-linux-gnu/libfoo.so.1 - would point out of the sysroot into
# whatever machine it was read on, so each one is rewritten relative to where it
# is, and one that points at something left behind is removed.
def _deb_sysroot_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output("sysroot", dir = True)
    script = """
set -eu
out="$1"
shift
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
    usr/lib/x86_64-linux-gnu usr/lib/aarch64-linux-gnu; do
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
find . -name '*\\*' -exec rm -rf {} +
"""
    if ctx.attrs.keep_xkb:
        script = script.replace("usr/lib/aarch64-linux-gnu; do", "usr/lib/aarch64-linux-gnu usr/share/X11/xkb; do")
    ctx.actions.run(
        cmd_args("sh", "-c", script, "--", out.as_output(), ctx.attrs.packages),
        category = "deb_sysroot",
    )
    return [DefaultInfo(default_output = out)]

# keep_xkb also keeps the XKB keymaps, which nothing compiles against but a
# program that builds a keyboard with libxkbcommon reads when it runs.
deb_sysroot = rule(
    attrs = {
        "keep_xkb": attrs.bool(default = False),
        "packages": attrs.list(attrs.source()),
    },
    impl = _deb_sysroot_impl,
)

# pkg-config, answering from the sysroot rather than from the machine.
#
# A -sys crate's build script asks pkg-config how to compile and link against
# a system library, through the pkg-config crate, which runs whatever $PKG_CONFIG
# names. This is that: a script that points the worker's own pkg-config at the
# sysroot's .pc files and prefixes every path it answers with the sysroot. The
# path to the sysroot is written relative to the script itself, because a build
# script runs in a directory of its own. The third-party fixups name it with
# PKG_CONFIG, and ask for rustc_link_lib so that the libraries it names reach the
# link, where --sysroot finds them.
def _pkg_config_impl(ctx: AnalysisContext) -> list[Provider]:
    script = ctx.actions.declare_output("pkg-config")
    sysroot = ctx.attrs.sysroot[DefaultInfo].default_outputs[0]
    ctx.actions.write(
        script,
        [
            "#!/bin/sh",
            cmd_args(sysroot, format = 'sysroot="$(cd "$(dirname "$0")/{}" && pwd)"', relative_to = (script, 1)),
            'export PKG_CONFIG_SYSROOT_DIR="$sysroot"',
            'export PKG_CONFIG_LIBDIR="$sysroot/usr/lib/{0}/pkgconfig:$sysroot/usr/share/pkgconfig"'.format(ctx.attrs.triple),
            "unset PKG_CONFIG_PATH",
            'exec pkg-config "$@"',
        ],
        is_executable = True,
    )
    return [
        DefaultInfo(default_output = script, other_outputs = [sysroot]),
        RunInfo(args = cmd_args(script, hidden = sysroot)),
    ]

pkg_config = rule(
    attrs = {
        "sysroot": attrs.dep(),
        "triple": attrs.string(),
    },
    impl = _pkg_config_impl,
)
