load("@root//buck/platforms:profile.bzl", "PROFILE_REFS", "keep_profile")
load(":package.bzl", _android_apk = "android_apk")

# Workers produce aligned, unsigned APK templates. RunInfo finalizes metadata
# and signs on the invoking machine; no key is a rule attribute or action input.
# The template transitions to Android once and is shared by every host; the
# RunInfo wrapper keeps its local runner on the requesting platform.

def _android_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.android[PlatformInfo], refs)

android_transition = transition(
    impl = _android_transition_impl,
    refs = {"android": "root//buck/platforms:android_arm64"} | PROFILE_REFS,
)

def _android_template_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output(ctx.label.name + "-unsigned.apk")
    library = ctx.attrs.library[DefaultInfo].sub_targets["cdylib"][DefaultInfo].default_outputs[0]
    command = cmd_args(
        ctx.attrs._apk[RunInfo],
        "apk",
        out.as_output(),
        "--jdk",
        ctx.attrs._jdk,
        "--build-tools",
        ctx.attrs._build_tools,
        "--android-jar",
        ctx.attrs._platform.project("android.jar"),
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
        cmd_args(ctx.attrs._ndk.project("sysroot/usr/lib/aarch64-linux-android/libc++_shared.so"), format = "--library={}"),
    )
    for source in ctx.attrs.java:
        command.add(cmd_args(source, format = "--java={}"))
    if ctx.attrs.resources:
        command.add(cmd_args(ctx.attrs.resources, format = "--resources={}"))
    if ctx.attrs.assets:
        command.add(cmd_args(ctx.attrs.assets[DefaultInfo].default_outputs[0], format = "--assets={}"))
    ctx.actions.run(command, category = "apk")
    return [DefaultInfo(default_output = out)]

android_template = rule(
    attrs = {
        "application_id": attrs.string(),
        "assets": attrs.option(attrs.dep(), default = None),
        "java": attrs.list(attrs.source(allow_directory = True), default = []),
        "label": attrs.string(),
        "library": attrs.dep(),
        "manifest": attrs.source(),
        "min_sdk": attrs.int(),
        "package": attrs.string(),
        "resources": attrs.option(attrs.source(allow_directory = True), default = None),
        "target_sdk": attrs.int(),
        "version_code": attrs.int(),
        "version_name": attrs.string(),
        "_apk": attrs.default_only(attrs.exec_dep(default = "root//crates/buck-tools:bin", providers = [RunInfo])),
        "_build_tools": attrs.default_only(attrs.source(default = "root//buck/android:build-tools")),
        "_jdk": attrs.default_only(attrs.source(default = "root//buck/android:jdk")),
        "_ndk": attrs.default_only(attrs.source(default = "root//buck/tools:android-ndk")),
        "_platform": attrs.default_only(attrs.source(default = "root//buck/android:platform")),
    },
    cfg = android_transition,
    impl = _android_template_impl,
)

def android_apk(name, activity, signing = "dev", visibility = None, **kwargs):
    android_template(name = name + "-template", **kwargs)
    _android_apk(
        name = name,
        activity = activity,
        application_id = kwargs["application_id"],
        signing = signing,
        template = ":" + name + "-template",
        visibility = visibility or ["PUBLIC"],
    )
