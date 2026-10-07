load("@prelude//cfg/modifier:cfg_constructor.bzl", "cfg_constructor_post_constraint_analysis")

# The prelude's modifiers, with the configuration named after the platform it
# was made from: the prelude only knows how to name Meta's own constraints, and
# calls every other configuration cfg:<empty>. -m release is the one modifier
# here, and its configuration is <platform>_release, the name keep_profile
# (profile.bzl) gives the configurations a transition makes from it.
def cfg_constructor_named(*, refs: dict[str, ProviderCollection], params) -> PlatformInfo:
    platform = cfg_constructor_post_constraint_analysis(params = params, refs = refs)
    if not platform.label.startswith("cfg:") or params.legacy_platform == None:
        return platform
    label = params.legacy_platform.label
    release = refs["root//buck/constraints:release"][ConstraintValueInfo] if "root//buck/constraints:release" in refs else None
    if release and platform.configuration.constraints.get(release.setting.label) == release:
        label += "_release"
    return PlatformInfo(configuration = platform.configuration, label = label)
