load("@root//buck/platforms:profile.bzl", "PROFILE_REFS", "keep_profile")

# What ./scripts/deploy runs: local-tools deploy, handed the server, the be
# bundles and the app's downloads, each built for the platform it runs on
# whatever machine deploys them, in the profile the command asks for
# (-m release). Each download is zipped on a worker by buck-tools archive under
# a folder of its own name, so only the zips come back here, and the APK is
# finalized and signed locally before upload.

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

def _macos_arm64_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.macos_arm64[PlatformInfo], refs)

_macos_arm64 = transition(
    impl = _macos_arm64_impl,
    refs = {"macos_arm64": "root//buck/platforms:macos_arm64"} | PROFILE_REFS,
)

def _macos_x86_64_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.macos_x86_64[PlatformInfo], refs)

_macos_x86_64 = transition(
    impl = _macos_x86_64_impl,
    refs = {"macos_x86_64": "root//buck/platforms:macos_x86_64"} | PROFILE_REFS,
)

def _windows_arm64_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.windows_arm64[PlatformInfo], refs)

_windows_arm64 = transition(
    impl = _windows_arm64_impl,
    refs = {"windows_arm64": "root//buck/platforms:windows_arm64"} | PROFILE_REFS,
)

def _windows_x86_64_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.windows_x86_64[PlatformInfo], refs)

_windows_x86_64 = transition(
    impl = _windows_x86_64_impl,
    refs = {"windows_x86_64": "root//buck/platforms:windows_x86_64"} | PROFILE_REFS,
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
    apps = [(name.replace("_", "-", 1), getattr(ctx.attrs, "app_" + name)) for name in _NATIVE]
    if ctx.attrs.web_download:
        apps.append(("web", ctx.attrs.web))
    for name, bundle in apps:
        if not bundle:
            continue
        folder = "block-app-" + name
        archive = ctx.actions.declare_output(folder + ".zip")
        ctx.actions.run(
            cmd_args(ctx.attrs._archive[RunInfo], "archive", archive.as_output(), folder, bundle[DefaultInfo].default_outputs[0]),
            category = "archive",
            identifier = name,
        )
        command.add("--download", cmd_args(name, "=", archive, delimiter = ""))
    if ctx.attrs.app_android:
        apk = ctx.attrs.app_android[DefaultInfo].default_outputs[0]
        command.add("--download", cmd_args("android=", apk, delimiter = ""))
        command.add("--apk-signer", ctx.attrs._signer, "--apk-metadata", ctx.attrs.be_linux_x86_64[DefaultInfo].default_outputs[0].project("build-info.json"))
    return [DefaultInfo(), RunInfo(args = command)]

_NATIVE = ["linux_x86_64", "linux_arm64", "macos_arm64", "macos_x86_64", "windows_x86_64", "windows_arm64"]

deploy = rule(
    attrs = {
        # The unsigned APK template, finalized locally with the release key, which builds
        # for Android whatever the command is configured for. It and the
        # other downloads are left out when ./scripts/deploy skips them, so
        # that nothing builds them.
        "app_android": attrs.option(attrs.dep(), default = None),
        "app_linux_arm64": attrs.option(attrs.transition_dep(cfg = _linux_arm64), default = None),
        "app_linux_x86_64": attrs.option(attrs.transition_dep(cfg = _linux_x86_64), default = None),
        "app_macos_arm64": attrs.option(attrs.transition_dep(cfg = _macos_arm64), default = None),
        "app_macos_x86_64": attrs.option(attrs.transition_dep(cfg = _macos_x86_64), default = None),
        "app_windows_arm64": attrs.option(attrs.transition_dep(cfg = _windows_arm64), default = None),
        "app_windows_x86_64": attrs.option(attrs.transition_dep(cfg = _windows_x86_64), default = None),
        "be_linux_arm64": attrs.transition_dep(cfg = _linux_arm64),
        "be_linux_x86_64": attrs.transition_dep(cfg = _linux_x86_64),
        "server": attrs.transition_dep(cfg = _linux_x86_64),
        # Run here, so built for this machine like the command itself.
        "tool": attrs.dep(providers = [RunInfo]),
        # The web bundle with its plugins, which builds for wasm whatever the
        # command is configured for.
        "web": attrs.dep(),
        # Whether the web bundle is zipped as a download too.
        "web_download": attrs.bool(default = True),
        "_archive": attrs.default_only(attrs.exec_dep(default = "root//crates/buck-tools:bin", providers = [RunInfo])),
        "_signer": attrs.default_only(attrs.source(default = "root//buck/android:apksign-wasm")),
    },
    impl = _deploy_impl,
)
