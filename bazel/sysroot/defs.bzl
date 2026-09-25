load("//bazel/tools:defs.bzl", "fetch")

# The packages laid out as a sysroot: headers, libraries (with GTK's and
# WebKitGTK's loadable modules), the gcc install clang takes the C++ runtime
# from, and pkg-config files. /lib and /lib64 link into usr/, as on a merged-/usr
# root, and absolute symlinks are made relative so they resolve inside it; one
# pointing at something left behind is removed. keep_xkb also keeps the XKB
# keymaps, which nothing compiles against but a program that builds a keyboard
# with libxkbcommon reads when it runs.
_SCRIPT = r"""
unpacked="$(mktemp -d)"
for package in "$downloads"/*.deb; do dpkg-deb -x "$package" "$unpacked"; done
for directory in lib lib64; do
    if [ -d "$unpacked/$directory" ] && [ ! -L "$unpacked/$directory" ]; then
        mkdir -p "$unpacked/usr/$directory"
        cp -a "$unpacked/$directory/." "$unpacked/usr/$directory/"
    fi
done
mkdir -p "$out/usr/lib"
for kept in usr/include usr/lib64 usr/lib/gcc usr/lib/pkgconfig usr/share/pkgconfig \
    usr/lib/x86_64-linux-gnu usr/lib/aarch64-linux-gnu KEPT; do
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
find . \( -type p -o -type s \) -delete
"""

def deb_sysroot(name, packages, keep_xkb = False, **kwargs):
    fetch(
        name = name,
        files = {sha256: [url] for _, url, sha256, _ in packages},
        mnemonic = "Sysroot",
        script = _SCRIPT.replace("KEPT", "usr/share/X11/xkb" if keep_xkb else ""),
        **kwargs
    )

# pkg-config, answering from the sysroot of the platform being built for: the
# worker's pkg-config pointed at the sysroot's .pc files, with the sysroot's
# path relative to the script, since a build script runs in a directory of its
# own. The sysroot is built for the worker, so it is fetched once.
def _pkg_config_impl(ctx):
    sysroot = ctx.file.sysroot
    script = ctx.actions.declare_file(ctx.label.name)
    up = "/".join([".."] * len(script.dirname.split("/")))
    ctx.actions.write(
        script,
        "\n".join([
            "#!/bin/sh",
            'sysroot="$(cd "$(dirname "$0")/{}/{}" && pwd)"'.format(up, sysroot.path),
            'export PKG_CONFIG_SYSROOT_DIR="$sysroot"',
            'export PKG_CONFIG_LIBDIR="$sysroot/usr/lib/{0}/pkgconfig:$sysroot/usr/share/pkgconfig"'.format(ctx.attr.triple),
            "unset PKG_CONFIG_PATH",
            'exec pkg-config "$@"',
            "",
        ]),
        is_executable = True,
    )
    return [DefaultInfo(
        executable = script,
        files = depset([script, sysroot]),
        runfiles = ctx.runfiles([sysroot]),
    )]

pkg_config = rule(
    implementation = _pkg_config_impl,
    attrs = {
        "sysroot": attr.label(allow_single_file = True, cfg = "exec"),
        "triple": attr.string(),
    },
    executable = True,
)

# Resolves the package closure on a worker: the snapshot's package indexes are
# downloaded for each architecture, and bazel-tools resolves what each set asks
# for into packages.bzl, which `./scripts/bazel run //:lock-sysroot` copies
# into place. Every architecture comes from the one archive: the snapshot of
# ubuntu-ports refuses anonymous requests, and the main archive's has the same
# packages at the same versions. The snapshot never changes, so the action's
# key - the snapshot and the sets - is all it depends on.
_SUITES = ["noble", "noble-updates", "noble-security"]

_COMPONENTS = ["main", "universe"]

def _deb_lock_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".bzl")
    base = "https://snapshot.ubuntu.com/ubuntu/" + ctx.attr.snapshot
    indexes = " ".join(["dists/{}/{}".format(suite, component) for suite in _SUITES for component in _COMPONENTS])
    script = """
set -eu
out="$1"; resolver="$2"; base="$3"; shift 3
scratch="$(mktemp -d)"
for set in "$@"; do
    architecture="$(echo "$set" | cut -d: -f2)"
    directory="$scratch/$architecture"
    [ -d "$directory" ] && continue
    mkdir -p "$directory"
    number=0
    for index in INDEXES; do
        curl --fail --silent --show-error --location --retry 5 \\
            --output "$directory/$number.xz" "$base/$index/binary-$architecture/Packages.xz"
        xz --decompress "$directory/$number.xz"
        number=$((number + 1))
    done
done
"$resolver" resolve "$base" "$scratch" "$@" > "$out"
rm -rf "$scratch"
""".replace("INDEXES", indexes)
    ctx.actions.run_shell(
        outputs = [out],
        tools = [ctx.executable._resolver],
        command = script,
        arguments = [out.path, ctx.executable._resolver.path, base] + [
            "{}:{}:{}".format(name, packages[0], ",".join(packages[1:]))
            for name, packages in ctx.attr.sets.items()
        ],
        mnemonic = "DebLock",
        execution_requirements = {"requires-network": "1"},
    )
    return [DefaultInfo(files = depset([out]))]

deb_lock = rule(
    implementation = _deb_lock_impl,
    attrs = {
        # name: [architecture, package...]
        "sets": attr.string_list_dict(),
        "snapshot": attr.string(),
        "_resolver": attr.label(default = "//crates/bazel-tools:bazel-tools-bin", executable = True, cfg = "exec"),
    },
)
