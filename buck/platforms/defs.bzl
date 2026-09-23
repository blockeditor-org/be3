load("@prelude//cfg/exec_platform:marker.bzl", "get_exec_platform_marker")

# The prelude's own execution platform, pointed at a remote executor.
#
# This is prelude//platforms:defs.bzl's execution_platform with the executor
# changed. Everything else about it is the same, and deliberately so: the cpu
# and os constraints are what a build script's exec dep is configured against,
# and the exec marker is what buck2 uses to tell an execution configuration
# from a target one.
#
# An action runs on the remote executor, in the container
# remote_execution_properties names, unless the rule that declared it asked to
# run locally. Local execution stays enabled for those; limited hybrid means
# buck2 never races the two. See guides/buck2.md.
def _execution_platform_impl(ctx: AnalysisContext) -> list[Provider]:
    constraints = dict()
    constraints.update(ctx.attrs.cpu_configuration[ConfigurationInfo].constraints)
    constraints.update(ctx.attrs.os_configuration[ConfigurationInfo].constraints)
    configuration = ConfigurationInfo(constraints = constraints, values = {})

    name = ctx.label.raw_target()
    platform = ExecutionPlatformInfo(
        configuration = configuration,
        executor_config = CommandExecutorConfig(
            allow_cache_uploads = False,
            local_enabled = True,
            remote_cache_enabled = True,
            remote_enabled = True,
            remote_execution_properties = ctx.attrs.remote_execution_properties,
            remote_execution_use_case = "buck2-default",
            use_limited_hybrid = True,
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
        "cpu_configuration": attrs.dep(providers = [ConfigurationInfo]),
        "os_configuration": attrs.dep(providers = [ConfigurationInfo]),
        "remote_execution_properties": attrs.dict(key = attrs.string(), value = attrs.string()),
    },
    impl = _execution_platform_impl,
)
