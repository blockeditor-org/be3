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
