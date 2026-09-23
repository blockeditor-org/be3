load("@prelude//cfg/exec_platform:marker.bzl", "get_exec_platform_marker")

# The prelude's own execution platform, with the remote cache turned on.
#
# This is prelude//platforms:defs.bzl's execution_platform with two fields
# added. Everything else about it is the same, and deliberately so: the host's
# cpu and os constraints are what a build script's exec dep is configured
# against, and the exec marker is what buck2 uses to tell an execution
# configuration from a target one.
#
# With remote_execution off, which is the default, nothing runs on a remote
# worker: what the cache gives us is the result of an action someone else
# already ran. With it on, an action runs on the remote executor, in the
# container remote_execution_properties names, unless the rule that declared it
# asked to run locally. See guides/buck2.md.
def _execution_platform_impl(ctx: AnalysisContext) -> list[Provider]:
    constraints = dict()
    constraints.update(ctx.attrs.cpu_configuration[ConfigurationInfo].constraints)
    constraints.update(ctx.attrs.os_configuration[ConfigurationInfo].constraints)
    configuration = ConfigurationInfo(constraints = constraints, values = {})

    name = ctx.label.raw_target()
    platform = ExecutionPlatformInfo(
        configuration = configuration,
        executor_config = CommandExecutorConfig(
            allow_cache_uploads = ctx.attrs.allow_cache_uploads,
            local_enabled = True,
            remote_cache_enabled = ctx.attrs.remote_cache_enabled,
            remote_enabled = ctx.attrs.remote_execution,
            # Only when remote: the properties are part of every action's
            # digest, so setting them for a local build would give it cache
            # keys of its own.
            remote_execution_properties = ctx.attrs.remote_execution_properties if ctx.attrs.remote_execution else {},
            remote_execution_use_case = "buck2-default",
            use_limited_hybrid = True,
            use_windows_path_separators = ctx.attrs.use_windows_path_separators,
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
        "allow_cache_uploads": attrs.bool(),
        "cpu_configuration": attrs.dep(providers = [ConfigurationInfo]),
        "os_configuration": attrs.dep(providers = [ConfigurationInfo]),
        "remote_cache_enabled": attrs.bool(),
        "remote_execution": attrs.bool(),
        "remote_execution_properties": attrs.dict(key = attrs.string(), value = attrs.string()),
        "use_windows_path_separators": attrs.bool(),
    },
    impl = _execution_platform_impl,
)

def _host_cpu_configuration() -> str:
    arch = host_info().arch
    if arch.is_aarch64:
        return "prelude//cpu:arm64"
    elif arch.is_arm:
        return "prelude//cpu:arm32"
    elif arch.is_i386:
        return "prelude//cpu:x86_32"
    elif arch.is_riscv64:
        return "prelude//cpu:riscv64"
    else:
        return "prelude//cpu:x86_64"

def _host_os_configuration() -> str:
    os = host_info().os
    if os.is_macos:
        return "prelude//os:macos"
    elif os.is_windows:
        return "prelude//os:windows"
    else:
        return "prelude//os:linux"

host_configuration = struct(
    cpu = _host_cpu_configuration(),
    os = _host_os_configuration(),
)
