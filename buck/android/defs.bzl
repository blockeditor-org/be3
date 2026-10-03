load("@root//buck/platforms:profile.bzl", "PROFILE_REFS", "keep_profile")

# An APK, made on a worker without Gradle by buck-tools apk, unsigned; sign.sh
# signs it here, with this machine's key or with CI's. An APK is only ever
# Android's, so the rule moves itself there: asked for under any target
# platform, it is the one configured target, and building //crates/... for
# every platform makes one APK rather than one for each.
# The NDK's libc++_shared.so goes beside the native library.

def _android_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.android[PlatformInfo], refs)

android_transition = transition(
    impl = _android_transition_impl,
    refs = {"android": "root//buck/platforms:android_arm64"} | PROFILE_REFS,
)

def _android_apk_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output(ctx.label.name + ".apk")
    library = ctx.attrs.library[DefaultInfo].sub_targets["cdylib"][DefaultInfo].default_outputs[0]
    command = cmd_args(
        ctx.attrs._apk[RunInfo],
        "apk",
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
    if ctx.attrs.resources:
        command.add(cmd_args(ctx.attrs.resources, format = "--resources={}"))
    if ctx.attrs.assets:
        command.add(cmd_args(ctx.attrs.assets[DefaultInfo].default_outputs[0], format = "--assets={}"))
    ctx.actions.run(command, category = "apk")
    return [DefaultInfo(default_output = out)]

android_apk = rule(
    attrs = {
        "application_id": attrs.string(),
        "assets": attrs.option(attrs.dep(), default = None),
        "java": attrs.list(attrs.source(allow_directory = True), default = []),
        "label": attrs.string(),
        "library": attrs.transition_dep(cfg = android_transition),
        "manifest": attrs.source(),
        "min_sdk": attrs.int(),
        "package": attrs.string(),
        "resources": attrs.option(attrs.source(allow_directory = True), default = None),
        "target_sdk": attrs.int(),
        "version_code": attrs.int(),
        "version_name": attrs.string(),
        "_apk": attrs.default_only(attrs.exec_dep(default = "root//crates/buck-tools:buck-tools-bin", providers = [RunInfo])),
        "_build_tools": attrs.default_only(attrs.source(default = "root//buck/android:build-tools")),
        "_jdk": attrs.default_only(attrs.source(default = "root//buck/android:jdk")),
        "_ndk": attrs.default_only(attrs.source(default = "root//buck/tools:android-ndk")),
        "_platform": attrs.default_only(attrs.source(default = "root//buck/android:platform")),
    },
    cfg = android_transition,
    impl = _android_apk_impl,
)
