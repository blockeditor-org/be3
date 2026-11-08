load("@root//buck/platforms:profile.bzl", "PROFILE_REFS", "keep_profile", "with_modifiers")

# What ./scripts/deploy runs: local-tools deploy, handed the server, the be
# bundles and the app's downloads, each built for the platform it runs on
# whatever machine deploys them, in the profile the command asks for
# (-m release). Each download is zipped on a worker by buck-tools archive under
# a folder of its own name, so only the zips come back here, and the APK is
# finalized and signed locally before upload. The native app downloads are
# also linked with LTO (buck/constraints:lto), which only be-host's binary
# selects on; the server and `be` keep the release build's configuration.

def _to(name: str, lto: bool):
    def impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
        kept = keep_profile(platform, getattr(refs, name)[PlatformInfo], refs)
        return with_modifiers(kept, [refs.lto[ConstraintValueInfo]]) if lto else kept

    return transition(impl = impl, refs = {name: "root//buck/platforms:" + name} | PROFILE_REFS)

_linux_x86_64 = _to("linux_x86_64", lto = False)
_linux_arm64 = _to("linux_arm64", lto = False)

_NATIVE = ["linux_x86_64", "linux_arm64", "macos_arm64", "macos_x86_64", "windows_x86_64", "windows_arm64"]

# buck2 names a transition by the variable it is assigned to, so each has one.
_linux_x86_64_lto = _to("linux_x86_64", lto = True)
_linux_arm64_lto = _to("linux_arm64", lto = True)
_macos_arm64_lto = _to("macos_arm64", lto = True)
_macos_x86_64_lto = _to("macos_x86_64", lto = True)
_windows_arm64_lto = _to("windows_arm64", lto = True)
_windows_x86_64_lto = _to("windows_x86_64", lto = True)

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
        folder = "be-host-" + name
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
        command.add("--apk-metadata", ctx.attrs.be_linux_x86_64[DefaultInfo].default_outputs[0].project("build-info.json"))
    return [DefaultInfo(), RunInfo(args = command)]

deploy = rule(
    attrs = {
        # The unsigned APK template, finalized locally with the release key, which builds
        # for Android whatever the command is configured for. It and the
        # other downloads are left out when ./scripts/deploy skips them, so
        # that nothing builds them.
        "app_android": attrs.option(attrs.dep(), default = None),
        "app_linux_arm64": attrs.option(attrs.transition_dep(cfg = _linux_arm64_lto), default = None),
        "app_linux_x86_64": attrs.option(attrs.transition_dep(cfg = _linux_x86_64_lto), default = None),
        "app_macos_arm64": attrs.option(attrs.transition_dep(cfg = _macos_arm64_lto), default = None),
        "app_macos_x86_64": attrs.option(attrs.transition_dep(cfg = _macos_x86_64_lto), default = None),
        "app_windows_arm64": attrs.option(attrs.transition_dep(cfg = _windows_arm64_lto), default = None),
        "app_windows_x86_64": attrs.option(attrs.transition_dep(cfg = _windows_x86_64_lto), default = None),
        "be_linux_arm64": attrs.transition_dep(cfg = _linux_arm64),
        "be_linux_x86_64": attrs.transition_dep(cfg = _linux_x86_64),
        "server": attrs.transition_dep(cfg = _linux_x86_64),
        # Run here, so built for this machine like the command itself.
        "tool": attrs.dep(providers = [RunInfo]),
        # The web bundle with its apps, which builds for wasm whatever the
        # command is configured for.
        "web": attrs.dep(),
        # Whether the web bundle is zipped as a download too.
        "web_download": attrs.bool(default = True),
        "_archive": attrs.default_only(attrs.exec_dep(default = "root//crates/be-tool-buck:bin", providers = [RunInfo])),
    },
    impl = _deploy_impl,
)
