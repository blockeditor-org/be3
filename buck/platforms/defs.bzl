load("@prelude//cfg/exec_platform:marker.bzl", "get_exec_platform_marker")

# prelude//platforms:defs.bzl's execution_platform. With `remote`, actions run on
# the build server unless their rule asks to run locally; without it, every action
# runs on this machine (guides/running/local_buck2.md). `label` names the platform,
# and with it the exec configurations (`<label>#<hash>`) and so every output path,
# so a local platform that gives the remote one's label builds into the same paths.
def _execution_platform_impl(ctx: AnalysisContext) -> list[Provider]:
    constraints = dict()
    constraints.update(ctx.attrs.cpu_configuration[ConfigurationInfo].constraints)
    constraints.update(ctx.attrs.os_configuration[ConfigurationInfo].constraints)
    for value in ctx.attrs.constraints:
        constraints[value[ConstraintValueInfo].setting.label] = value[ConstraintValueInfo]
    configuration = ConfigurationInfo(constraints = constraints, values = {})

    name = (ctx.attrs.label or ctx.label).raw_target()
    remote = ctx.attrs.remote
    platform = ExecutionPlatformInfo(
        configuration = configuration,
        executor_config = CommandExecutorConfig(
            allow_cache_uploads = False,
            local_enabled = True,
            remote_cache_enabled = remote,
            remote_enabled = remote,
            remote_execution_properties = ctx.attrs.remote_execution_properties,
            remote_execution_use_case = "buck2-default",
            use_limited_hybrid = remote,
        ),
        label = name,
    )

    return [
        DefaultInfo(),
        platform,
        PlatformInfo(configuration = configuration, label = str(name)),
        ExecutionPlatformRegistrationInfo(
            exec_marker_constraint = get_exec_platform_marker(),
            platforms = [platform],
        ),
    ]

execution_platform = rule(
    attrs = {
        "constraints": attrs.list(attrs.dep(providers = [ConstraintValueInfo]), default = []),
        "cpu_configuration": attrs.dep(providers = [ConfigurationInfo]),
        "label": attrs.option(attrs.label(), default = None),
        "os_configuration": attrs.dep(providers = [ConfigurationInfo]),
        "remote": attrs.bool(default = True),
        "remote_execution_properties": attrs.dict(default = {}, key = attrs.string(), value = attrs.string()),
    },
    impl = _execution_platform_impl,
)

# Every execution platform, in the order buck2 tries them: the first that a
# target is compatible with runs its actions.
def _execution_platforms_impl(ctx: AnalysisContext) -> list[Provider]:
    return [
        DefaultInfo(),
        ExecutionPlatformRegistrationInfo(
            exec_marker_constraint = get_exec_platform_marker(),
            platforms = [platform[ExecutionPlatformInfo] for platform in ctx.attrs.platforms],
        ),
    ]

execution_platforms = rule(
    attrs = {"platforms": attrs.list(attrs.dep(providers = [ExecutionPlatformInfo]))},
    impl = _execution_platforms_impl,
)
