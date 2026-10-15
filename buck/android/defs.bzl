load("@root//buck/platforms:profile.bzl", "PROFILE_REFS", "keep_profile")

# An APK, made on a worker without Gradle by buck-tools apk. It is unsigned
# unless it names a keystore, which signs it on a worker; `buck2 run`
# on an unsigned one signs it here with this machine's key (sign.sh) and
# --install installs it and starts `activity`. An APK is only ever Android's,
# so the rule moves itself there: asked for under any target platform, it is the
# one configured target, and building //crates/... for every platform makes one
# APK rather than one for each.
# The NDK's libc++_shared.so goes beside the native library. Only the files
# it reads from the platform and the NDK are its inputs, android.jar and
# libc++_shared.so, rather than their 15,000 others, each of which buck2 would
# otherwise ask the build server about before it ran the action.

def _android_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.android[PlatformInfo], refs)

android_transition = transition(
    impl = _android_transition_impl,
    refs = {"android": "root//buck/platforms:android_arm64"} | PROFILE_REFS,
)

_sign = """
set -eu
jdk="$1" build_tools="$2" unsigned="$3" out="$4" keystore_dir="$5" name="$6"
if [ ! -f "$keystore_dir/$name" ]; then
    echo "There is no buck/android/$name to sign with: guides/build_server.md says where CI's comes from, and ./scripts/deploy writes the release one." >&2
    exit 1
fi
keystore="$(mktemp)"
base64 -d < "$keystore_dir/$name" > "$keystore"
JAVA_HOME="$jdk" "$jdk/bin/java" -jar "$build_tools/lib/apksigner.jar" sign \
    --ks "$keystore" --ks-pass pass:android --in "$unsigned" --out "$out"
rm -f "$keystore"
"""

# The APK signed on a worker with the keystore named, buck/android:ci-keystore
# or :release-keystore, a base64 keystore whose password is `android`. The
# keystore is an input like any other, so a new one signs afresh.
def _sign_with(ctx: AnalysisContext, unsigned: Artifact) -> Artifact:
    out = ctx.actions.declare_output(ctx.label.name + ".apk")
    ctx.actions.run(
        cmd_args(
            "sh",
            "-c",
            _sign,
            "sh",
            ctx.attrs._jdk,
            ctx.attrs._build_tools,
            unsigned,
            out.as_output(),
            ctx.attrs.keystore[DefaultInfo].default_outputs[0],
            ctx.attrs.keystore.label.name + ".base64",
        ),
        category = "apk_sign",
    )
    return out

def _android_apk_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output(ctx.label.name + ("-unsigned.apk" if ctx.attrs.keystore else ".apk"))
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
    if ctx.attrs.keystore:
        return [DefaultInfo(default_output = _sign_with(ctx, out))]
    run = cmd_args(
        "sh",
        ctx.attrs._sign,
        ctx.attrs._jdk,
        ctx.attrs._build_tools,
        ctx.attrs._platform_tools,
        out,
        ctx.label.package.split("/")[-1],
        ctx.attrs.activity,
    )
    return [DefaultInfo(default_output = out), RunInfo(args = run)]

android_apk = rule(
    attrs = {
        # The activity --install starts, given the package's name.
        "activity": attrs.string(),
        "application_id": attrs.string(),
        "assets": attrs.option(attrs.dep(), default = None),
        "java": attrs.list(attrs.source(allow_directory = True), default = []),
        "keystore": attrs.option(attrs.dep(), default = None),
        "label": attrs.string(),
        "library": attrs.transition_dep(cfg = android_transition),
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
        "_platform_tools": attrs.default_only(attrs.source(default = "root//buck/android:platform-tools")),
        "_sign": attrs.default_only(attrs.source(default = "root//buck/android:sign.sh")),
    },
    cfg = android_transition,
    impl = _android_apk_impl,
)
