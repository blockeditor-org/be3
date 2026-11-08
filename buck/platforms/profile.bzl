# What every transition in the build does with the platform it replaces: the
# target platform changes, the profile (buck/constraints:release) does not, so
# a release build's apps and Android library are release builds too.
PROFILE_REFS = {"release": "root//buck/constraints:release"}

def keep_profile(incoming: PlatformInfo, target: PlatformInfo, refs: struct) -> PlatformInfo:
    setting = refs.release[ConstraintValueInfo].setting.label
    value = incoming.configuration.constraints.get(setting)
    if value == None:
        return target
    constraints = dict(target.configuration.constraints)
    constraints[setting] = value
    return PlatformInfo(
        configuration = ConfigurationInfo(constraints = constraints, values = target.configuration.values),
        label = target.label + "_release",
    )

# Flags for cargo's dev profile alone, which is every build but a release one:
# the [profile.dev.package] overrides in Cargo.toml, which crates.bzl carries
# for each crate they name.
def dev_only(flags):
    if not flags:
        return []
    return select({
        "DEFAULT": flags,
        "root//buck/constraints:release": [],
    })

# Every opt level the build sets, by what is compiled: (cargo's dev profile on
# the host, the dev profile on wasm, the release profile), None where the kind
# keeps the toolchain's. The toolchain's applies to every crate, third-party
# ones included; the rest are added to a first-party crate's own flags, after
# the toolchain's, which they override.
#
# - toolchain: cargo's dev profile on the host, its app profile on wasm, since
#   cranelift is far slower at an unoptimised module, and its release profile.
#   The opt level is also what tells build scripts OPT_LEVEL.
# - member, every workspace crate: on wasm outside a release build, 1 rather
#   than the app profile's 2. A change under beui recompiles each of them for
#   the apps, and at 2 that took a third of the CPU ./scripts/verify spent on a
#   change to geometry, for modules no slower to precompile or test.
# - app, an app's own crate (app_module in buck/wasm/defs.bzl): 0 outside a
#   release build. Most of an app's code is beui's generics instantiated anew,
#   so a change under beui recompiles every app, and at 2 LLVM made that the
#   longest part of ./scripts/verify: thirty apps took 1,130 seconds of
#   compiling at 2 and 270 at 0. The modules are about two and a half times the
#   size and take half as long again to precompile, and the app tests ran no
#   slower.
# - app_test, a crate's tests compiled to wasm (app_tests): 0 in every profile.
#   Every module instantiates beui's generics anew, so a change to beui
#   recompiles about thirty of them at once, and at 2 LLVM is most of that: the
#   step took twice as long, though the tests run faster and the modules are
#   smaller to precompile.
# - fuzz, a fuzz target and the library under it (cargo_fuzz): optimised, as
#   cargo-fuzz builds one.
OPT_LEVELS = {
    "app": (None, 0, None),
    "app_test": (None, 0, 0),
    "fuzz": (3, 3, 3),
    "member": (None, 1, None),
    "toolchain": (0, 2, 3),
}

def _opt_flag(level):
    return [] if level == None else ["-Copt-level={}".format(level)]

# A kind's opt level other than the toolchain's, as flags.
def opt_flags(kind):
    host, wasm, release = OPT_LEVELS[kind]
    if host == wasm and wasm == release:
        return _opt_flag(host)
    return select({
        "DEFAULT": select({
            "DEFAULT": _opt_flag(host),
            "prelude//cpu:wasm32": _opt_flag(wasm),
        }),
        "root//buck/constraints:release": _opt_flag(release),
    })

# A target in cargo's release profile whatever the command asked for, for what
# a command downloads: crates/be-tool-verify, which it runs here, and the paint review
# page, a quarter the size of its dev build.
def _release_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    release = refs.release[ConstraintValueInfo]
    if platform.configuration.constraints.get(release.setting.label) == release:
        return platform
    constraints = dict(platform.configuration.constraints)
    constraints[release.setting.label] = release
    return PlatformInfo(
        configuration = ConfigurationInfo(constraints = constraints, values = platform.configuration.values),
        label = platform.label + "_release",
    )

_release = transition(impl = _release_impl, refs = PROFILE_REFS)

def _released_impl(ctx: AnalysisContext) -> list[Provider]:
    actual = ctx.attrs.actual
    return [actual[DefaultInfo]] + ([actual[RunInfo]] if RunInfo in actual else [])

released = rule(
    attrs = {"actual": attrs.transition_dep(cfg = _release)},
    impl = _released_impl,
)

# Every crate's own flags in a release build (they cannot be the toolchain's,
# whose rustc_cfg runs rustc without nightly options). Legacy mangling names a
# generic instance by its path and a hash rather than by every type argument, so
# a symbol is a third the length: 16 MB less in be-host, whose backtraces
# still name each function, and a seventh off the apps, whose name sections
# and exports are made of these symbols.
RELEASE_SYMBOL_FLAGS = select({
    "DEFAULT": [],
    "root//buck/constraints:release": ["-Zunstable-options", "-Csymbol-mangling-version=legacy"],
})

# Whether every action runs on this machine: .buckconfig.local names this list
# of execution platforms when no build server is reachable
# (guides/running/local_buck2.md). Nothing else tells a local build apart.
LOCAL_BUILD = read_root_config("build", "execution_platforms", "") == "root//buck/platforms:execution_platforms_local"

# The threads rustc's frontend runs on. The flag is on every first-party rustc
# command line, so it is part of every cache key: on a build server it is eight
# whoever asks, and be3.rustc_threads is read only when the build is local, where
# nothing is shared.
RUSTC_THREADS = "-Zthreads=" + (read_root_config("be3", "rustc_threads", "8") if LOCAL_BUILD else "8")

# What a rust target takes on a local build. The prelude's rust rules name their
# outputs by content, which lets a build server share them, and buck2 then writes
# each output where the action ran and copies it to the path named for its hash,
# leaving two copies that are a third of buck-out. Spread into every rust target;
# empty on a build server, so nothing there changes.
RUST_LOCAL = {"use_content_based_paths": False} if LOCAL_BUILD else {}
