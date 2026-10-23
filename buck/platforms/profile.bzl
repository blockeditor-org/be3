# What every transition in the build does with the platform it replaces: the
# target platform changes, the profile (buck/constraints:release) does not, so
# a release build's plugins and Android library are release builds too.
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

# A binary in cargo's release profile whatever the command asked for, for a
# tool a command downloads to run here: crates/verify is a few hundred
# kilobytes so, rather than megabytes of debug information.
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

released_binary = rule(
    attrs = {"actual": attrs.transition_dep(cfg = _release, providers = [RunInfo])},
    impl = lambda ctx: [ctx.attrs.actual[DefaultInfo], ctx.attrs.actual[RunInfo]],
)
