load("@root//buck/tools:defs.bzl", "configured_once")

# The rules behind buck/sysroot/BUCK.

# Resolves the package closure: buck-tools resolves what each set asks for
# against the snapshot's package indexes for its architecture (deb_indexes)
# into packages.bzl, which ./scripts/verify copies into place.
# Every architecture comes from the one archive: the snapshot of ubuntu-ports
# refuses anonymous requests, and the main archive's has the same packages at
# the same versions.
def _deb_lock_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output("packages.bzl")
    base = "https://snapshot.ubuntu.com/ubuntu/" + ctx.attrs.snapshot
    ctx.actions.run(
        cmd_args(
            ctx.attrs._resolver[RunInfo],
            "resolve",
            out.as_output(),
            base,
            [cmd_args(key.split("/")[-1], index, delimiter = "=") for key, index in sorted(ctx.attrs.indexes.items())],
            "--",
            ["{}:{}:{}".format(name, architecture, ",".join(packages)) for name, (architecture, packages) in ctx.attrs.sets.items()],
        ),
        category = "deb_lock",
    )
    return [DefaultInfo(default_output = out)]

deb_lock = rule(
    attrs = {
        "indexes": attrs.dict(attrs.string(), attrs.source()),
        "sets": attrs.dict(attrs.string(), attrs.tuple(attrs.string(), attrs.list(attrs.string()))),
        "snapshot": attrs.string(),
        "_resolver": attrs.default_only(attrs.exec_dep(default = "root//crates/be-tool-buck:bin", providers = [RunInfo])),
    },
    impl = _deb_lock_impl,
)

# The packages laid out as a sysroot: headers, libraries, the gcc install clang takes the C++ runtime
# from, and pkg-config files. /lib and /lib64 link into usr/, as on a merged-/usr
# root, and absolute symlinks are made relative so they resolve inside it; one
# pointing at something left behind is removed.
def _deb_sysroot_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output("sysroot", dir = True)
    command = cmd_args("sh", ctx.attrs._script, out.as_output())
    if ctx.attrs.keep_xkb:
        command.add("--keep-xkb")
    for program in ctx.attrs.programs:
        command.add("--keep", program)
    command.add(ctx.attrs.packages)
    ctx.actions.run(command, category = "deb_sysroot")
    return [DefaultInfo(default_output = out)]

# keep_xkb also keeps the XKB keymaps, which nothing compiles against but a
# program that builds a keyboard with libxkbcommon reads when it runs, and
# programs the files a test runs, such as usr/bin/dbus-daemon.
deb_sysroot = rule(
    attrs = {
        "keep_xkb": attrs.bool(default = False),
        "packages": attrs.list(attrs.source()),
        "programs": attrs.list(attrs.string(), default = []),
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

_ARCH_PREFIXES = {
    "amd64": "",
    "arm64": "arm64-",
}

# The snapshot's package indexes, SUITE/COMPONENT/ARCHITECTURE to the sha256
# and size of its Packages.gz, each a download of its own: a snapshot never
# changes, so they are pinned like everything else and downloaded once rather
# than by every deb_lock action. The snapshot's dists/SUITE/InRelease lists
# them, under SHA256.
def deb_indexes(snapshot, indexes):
    for key, (sha256, size_bytes) in sorted(indexes.items()):
        suite, component, architecture = key.split("/")
        native.http_file(
            name = "index-{}-{}-{}".format(suite, component, architecture),
            sha256 = sha256,
            size_bytes = size_bytes,
            urls = ["https://snapshot.ubuntu.com/ubuntu/{}/dists/{}/{}/binary-{}/Packages.gz".format(snapshot, suite, component, architecture)],
        )
    return {key: ":index-" + key.replace("/", "-") for key in indexes}

# A package's download, by its set's architecture: the amd64 packages are named
# after the package, the arm64 ones are prefixed.
def _deb_name(set_name, name):
    return _ARCH_PREFIXES[set_name.split("-")[0]] + name.replace("+", "p") + ".deb"

# One http_file for each package of each architecture, whichever sets name it:
# the sets resolve against the one snapshot, so a package is the same file in
# every set of its architecture, and a sysroot that shares it with another does
# not download it again.
def deb_downloads(packages):
    downloads = {}
    for set_name in sorted(packages):
        for name, url, sha256, size_bytes in packages[set_name]:
            target = _deb_name(set_name, name)
            if target in downloads and downloads[target][0] != url:
                fail("{} is {} in one set and {} in another".format(target, downloads[target][0], url))
            downloads[target] = (url, sha256, size_bytes)
    for target, (url, sha256, size_bytes) in sorted(downloads.items()):
        native.http_file(name = target, sha256 = sha256, size_bytes = size_bytes, urls = [url])

# A set's sysroot, configured once (buck/tools/defs.bzl), so every
# configuration that compiles C or links shares its download and unpacking.
def sysroot(name, set_name, packages, **kwargs):
    configured_once(
        deb_sysroot,
        name = name,
        packages = [":" + _deb_name(set_name, package) for package, _, _, _ in packages.get(set_name, [])],
        **kwargs,
    )
