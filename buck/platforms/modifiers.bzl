load("@prelude//cfg/modifier:cfg_constructor.bzl", "cfg_constructor_post_constraint_analysis")
load(":profile.bzl", "MODIFIERS", "modifiers_of", "with_modifiers")

# The prelude's modifiers, with the configuration named after the platform it
# was made from: the prelude only knows how to name Meta's own constraints, and
# calls every other configuration cfg:<empty>. Each modifier in MODIFIERS
# (profile.bzl) appends its name, as a transition's with_modifiers does, so
# -m release on linux_x86_64 is linux_x86_64_release whichever made it.
def cfg_constructor_named(*, refs: dict[str, ProviderCollection], params) -> PlatformInfo:
    platform = cfg_constructor_post_constraint_analysis(params = params, refs = refs)
    if not platform.label.startswith("cfg:") or params.legacy_platform == None:
        return platform
    values = [refs[label][ConstraintValueInfo] for label in MODIFIERS.values() if label in refs]
    named = with_modifiers(params.legacy_platform, modifiers_of(platform.configuration, values))
    return PlatformInfo(configuration = platform.configuration, label = named.label)
