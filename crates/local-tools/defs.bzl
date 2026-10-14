load("@root//buck/platforms:profile.bzl", "PROFILE_REFS", "keep_profile")

# What ./scripts/deploy runs: local-tools deploy, handed the server and the be
# bundles built for the platforms they run on whatever machine deploys them,
# in the profile the command asks for (-m release).

def _linux_x86_64_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.linux_x86_64[PlatformInfo], refs)

_linux_x86_64 = transition(
    impl = _linux_x86_64_impl,
    refs = {"linux_x86_64": "root//buck/platforms:linux_x86_64"} | PROFILE_REFS,
)

def _linux_arm64_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.linux_arm64[PlatformInfo], refs)

_linux_arm64 = transition(
    impl = _linux_arm64_impl,
    refs = {"linux_arm64": "root//buck/platforms:linux_arm64"} | PROFILE_REFS,
)

def _deploy_impl(ctx: AnalysisContext) -> list[Provider]:
    server = ctx.attrs.server[DefaultInfo].default_outputs[0]
    command = cmd_args(
        ctx.attrs.tool[RunInfo],
        "deploy",
        "--server",
        server,
        "--be",
        cmd_args("x86_64-unknown-linux-gnu=", ctx.attrs.be_linux_x86_64[DefaultInfo].default_outputs[0], delimiter = ""),
        "--be",
        cmd_args("aarch64-unknown-linux-gnu=", ctx.attrs.be_linux_arm64[DefaultInfo].default_outputs[0], delimiter = ""),
        "--web",
        ctx.attrs.web[DefaultInfo].default_outputs[0],
    )
    return [DefaultInfo(), RunInfo(args = command)]

deploy = rule(
    attrs = {
        "be_linux_arm64": attrs.transition_dep(cfg = _linux_arm64),
        "be_linux_x86_64": attrs.transition_dep(cfg = _linux_x86_64),
        "server": attrs.transition_dep(cfg = _linux_x86_64),
        # Run here, so built for this machine like the command itself.
        "tool": attrs.dep(providers = [RunInfo]),
        # The web bundle with its plugins, which builds for wasm whatever the
        # command is configured for.
        "web": attrs.dep(),
    },
    impl = _deploy_impl,
)
