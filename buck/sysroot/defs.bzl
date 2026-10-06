# The rules behind buck/sysroot/BUCK.

# Resolves the package closure on a worker: the snapshot's package indexes
# are downloaded for each architecture, and buck-tools resolves what each set
# asks for into packages.bzl, which ./scripts/buck run //:lock-sysroot copies
# into place. Every architecture comes from the one archive: the snapshot of
# ubuntu-ports refuses anonymous requests, and the main archive's has the same
# packages at the same versions.
_suites = ["noble", "noble-updates", "noble-security"]

_components = ["main", "universe"]

def _deb_lock_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output("packages.bzl")
    base = "https://snapshot.ubuntu.com/ubuntu/" + ctx.attrs.snapshot
    indexes = " ".join(["dists/{}/{}".format(suite, component) for suite in _suites for component in _components])
    ctx.actions.run(
        cmd_args(
            "sh",
            ctx.attrs._script,
            out.as_output(),
            ctx.attrs._resolver[RunInfo],
            base,
            indexes,
            ["{}:{}:{}".format(name, architecture, ",".join(packages)) for name, (architecture, packages) in ctx.attrs.sets.items()],
        ),
        category = "deb_lock",
    )
    return [DefaultInfo(default_output = out)]

deb_lock = rule(
    attrs = {
        "sets": attrs.dict(attrs.string(), attrs.tuple(attrs.string(), attrs.list(attrs.string()))),
        "snapshot": attrs.string(),
        "_resolver": attrs.default_only(attrs.exec_dep(default = "root//crates/buck-tools:buck-tools-bin", providers = [RunInfo])),
        "_script": attrs.default_only(attrs.source(default = "root//buck/sysroot:deb_lock.sh")),
    },
    impl = _deb_lock_impl,
)

# The packages laid out as a sysroot: headers, libraries (with GTK's and
# WebKitGTK's loadable modules), the gcc install clang takes the C++ runtime
# from, and pkg-config files. /lib and /lib64 link into usr/, as on a merged-/usr
# root, and absolute symlinks are made relative so they resolve inside it; one
# pointing at something left behind is removed.
def _deb_sysroot_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output("sysroot", dir = True)
    command = cmd_args("sh", ctx.attrs._script, out.as_output())
    if ctx.attrs.keep_xkb:
        command.add("--keep-xkb")
    command.add(ctx.attrs.packages)
    ctx.actions.run(command, category = "deb_sysroot")
    return [DefaultInfo(default_output = out)]

# keep_xkb also keeps the XKB keymaps, which nothing compiles against but a
# program that builds a keyboard with libxkbcommon reads when it runs.
deb_sysroot = rule(
    attrs = {
        "keep_xkb": attrs.bool(default = False),
        "packages": attrs.list(attrs.source()),
        "_script": attrs.default_only(attrs.source(default = "root//buck/sysroot:deb_sysroot.sh")),
    },
    impl = _deb_sysroot_impl,
)

# pkg-config, answering from the sysroot: the worker's pkg-config pointed at the
# sysroot's .pc files, with the sysroot's path relative to the script, since a
# build script runs in a directory of its own. Fixups name it with PKG_CONFIG
# and rustc_link_lib, and --sysroot finds the libraries at the link.
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
