load("@root//buck/platforms:profile.bzl", "PROFILE_REFS", "keep_profile")

# An APK, made on a worker without Gradle by buck-tools apk, unsigned; sign.sh signs it
# locally with this machine's key, and signed_apk on a worker with CI's. An APK
# is only ever Android's, so both rules move themselves there: asked for under
# any target platform, they are the one configured target, and building
# //crates/... for every platform makes one APK rather than one for each.
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

_sign = """
set -eu
jdk="$1" build_tools="$2" unsigned="$3" out="$4" keystore_dir="$5"
if [ ! -f "$keystore_dir/ci-keystore.base64" ]; then
    echo 'There is no buck/android/ci-keystore.base64 to sign with: guides/build_server.md says where it comes from.' >&2
    exit 1
fi
keystore="$(mktemp)"
base64 -d < "$keystore_dir/ci-keystore.base64" > "$keystore"
JAVA_HOME="$jdk" "$jdk/bin/java" -jar "$build_tools/lib/apksigner.jar" sign \
    --ks "$keystore" --ks-pass pass:android --in "$unsigned" --out "$out"
rm -f "$keystore"
"""

# An APK signed on a worker with CI's keystore, buck/android:ci-keystore. The
# keystore is an input like any other, so a new one signs afresh.
def _signed_apk_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output(ctx.label.name + ".apk")
    ctx.actions.run(
        cmd_args(
            "sh",
            "-c",
            _sign,
            "sh",
            ctx.attrs._jdk,
            ctx.attrs._build_tools,
            ctx.attrs.apk[DefaultInfo].default_outputs[0],
            out.as_output(),
            ctx.attrs._keystore[DefaultInfo].default_outputs[0],
        ),
        category = "apk_sign",
    )
    return [DefaultInfo(default_output = out)]

signed_apk = rule(
    attrs = {
        "apk": attrs.dep(),
        "_build_tools": attrs.default_only(attrs.source(default = "root//buck/android:build-tools")),
        "_jdk": attrs.default_only(attrs.source(default = "root//buck/android:jdk")),
        "_keystore": attrs.default_only(attrs.dep(default = "root//buck/android:ci-keystore")),
    },
    cfg = android_transition,
    impl = _signed_apk_impl,
)
