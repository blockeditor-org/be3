# An APK, made on a worker without Gradle by apk.py, unsigned; sign.py signs it
# locally. Only the native library is built for Android, through the
# transition; the NDK's libc++_shared.so goes beside it.

def _android_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return refs.android[PlatformInfo]

android_transition = transition(
    impl = _android_transition_impl,
    refs = {"android": "root//buck/platforms:android_arm64"},
)

def _android_apk_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output(ctx.label.name + ".apk")
    library = ctx.attrs.library[DefaultInfo].sub_targets["cdylib"][DefaultInfo].default_outputs[0]
    command = cmd_args(
        "python3",
        ctx.attrs._apk,
        out.as_output(),
        "--jdk",
        ctx.attrs._jdk,
        "--build-tools",
        ctx.attrs._build_tools,
        "--platform",
        ctx.attrs._platform,
        "--manifest",
        ctx.attrs.manifest,
        "--package",
        ctx.attrs.package,
        "--application-id",
        ctx.attrs.application_id,
        "--label",
        ctx.attrs.label,
        "--min-sdk",
        str(ctx.attrs.min_sdk),
        "--target-sdk",
        str(ctx.attrs.target_sdk),
        "--version-code",
        str(ctx.attrs.version_code),
        "--version-name",
        ctx.attrs.version_name,
        cmd_args(library, format = "--library={}"),
        cmd_args(ctx.attrs._ndk, format = "--library={}/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so"),
    )
    for source in ctx.attrs.java:
        command.add(cmd_args(source, format = "--java={}"))
    if ctx.attrs.assets:
        command.add(cmd_args(ctx.attrs.assets[DefaultInfo].default_outputs[0], format = "--assets={}"))
    ctx.actions.run(command, category = "apk")
    return [DefaultInfo(default_output = out)]

android_apk = rule(
    attrs = {
        "application_id": attrs.string(),
        "assets": attrs.option(attrs.dep(), default = None),
        "java": attrs.list(attrs.source(), default = []),
        "label": attrs.string(),
        "library": attrs.transition_dep(cfg = android_transition),
        "manifest": attrs.source(),
        "min_sdk": attrs.int(),
        "package": attrs.string(),
        "target_sdk": attrs.int(),
        "version_code": attrs.int(),
        "version_name": attrs.string(),
        "_apk": attrs.default_only(attrs.source(default = "root//buck/android:apk.py")),
        "_build_tools": attrs.default_only(attrs.source(default = "root//buck/android:build-tools")),
        "_jdk": attrs.default_only(attrs.source(default = "root//buck/android:jdk")),
        "_ndk": attrs.default_only(attrs.source(default = "root//buck/tools:android-ndk")),
        "_platform": attrs.default_only(attrs.source(default = "root//buck/android:platform")),
    },
    impl = _android_apk_impl,
)
