# An APK, made on a worker without Gradle by buck-tools apk, unsigned; sign.sh signs it
# locally with this machine's key, and signed_apk on a worker with CI's. Only the native library is built for Android, through the
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
        "_apk": attrs.default_only(attrs.exec_dep(default = "root//crates/buck-tools:buck-tools-bin", providers = [RunInfo])),
        "_build_tools": attrs.default_only(attrs.source(default = "root//buck/android:build-tools")),
        "_jdk": attrs.default_only(attrs.source(default = "root//buck/android:jdk")),
        "_ndk": attrs.default_only(attrs.source(default = "root//buck/tools:android-ndk")),
        "_platform": attrs.default_only(attrs.source(default = "root//buck/android:platform")),
    },
    impl = _android_apk_impl,
)

_sign = """
set -eu
jdk="$1" build_tools="$2" unsigned="$3" out="$4"
if [ -z "${ANDROID_DEBUG_KEYSTORE_BASE64:-}" ]; then
    echo 'BuildBuddy passed no ANDROID_DEBUG_KEYSTORE_BASE64: add it under the organization secrets.' >&2
    exit 1
fi
keystore="$(mktemp)"
printf '%s' "$ANDROID_DEBUG_KEYSTORE_BASE64" | base64 -d > "$keystore"
JAVA_HOME="$jdk" "$jdk/bin/java" -jar "$build_tools/lib/apksigner.jar" sign \
    --ks "$keystore" --ks-pass pass:android --in "$unsigned" --out "$out"
rm -f "$keystore"
"""

# An APK signed on a worker with the keystore BuildBuddy keeps as a secret,
# which it passes only to actions on root//buck/platforms:android_signing: the
# target names root//buck/constraints:android_keystore in exec_compatible_with.
# The secret is not part of the action's key, so key_version is: bump it
# with the secret, or builds go on reusing APKs the old key signed.
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
            "key-version-{}".format(ctx.attrs.key_version),
        ),
        category = "apk_sign",
    )
    return [DefaultInfo(default_output = out)]

signed_apk = rule(
    attrs = {
        "apk": attrs.dep(),
        "key_version": attrs.int(),
        "_build_tools": attrs.default_only(attrs.source(default = "root//buck/android:build-tools")),
        "_jdk": attrs.default_only(attrs.source(default = "root//buck/android:jdk")),
    },
    impl = _signed_apk_impl,
)
