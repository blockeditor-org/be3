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

# What a rust target takes on a build with no build server (be3.build_server =
# none, guides/running/local_buck2.md). The prelude's rust rules name their outputs by content,
# which lets a build server share them, and buck2 then writes each output where the
# action ran and copies it to the path named for its hash, leaving two copies that
# are a third of buck-out. Spread into every rust target; empty on a build server, so
# nothing there changes.
RUST_LOCAL = {"use_content_based_paths": False} if read_root_config("be3", "build_server", "") == "none" else {}
