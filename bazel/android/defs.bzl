# An APK, made on a worker without Gradle by bazel-tools apk, unsigned; sign.sh
# signs it here with this machine's key, and signed_apk on a worker with CI's.
# An APK is only ever Android's, so both rules move themselves there: asked for
# under any target platform, they are the one configured target. The NDK's
# libc++_shared.so goes beside the native library.
def _android_transition_impl(_settings, _attr):
    return {"//command_line_option:platforms": str(Label("//bazel/platforms:android_arm64"))}

android_transition = transition(
    implementation = _android_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _android_apk_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".apk")
    library = [file for file in ctx.files.library if file.extension == "so"]
    if len(library) != 1:
        fail("{} is not one shared library".format(ctx.attr.library[0].label))
    arguments = ctx.actions.args()
    arguments.add("apk")
    arguments.add(out)
    arguments.add("--jdk", ctx.file._jdk.path)
    arguments.add("--build-tools", ctx.file._build_tools.path)
    arguments.add("--platform", ctx.file._platform.path)
    arguments.add("--manifest", ctx.file.manifest)
    arguments.add("--package", ctx.attr.package)
    arguments.add("--application-id", ctx.attr.application_id)
    arguments.add("--label", ctx.attr.label)
    arguments.add("--min-sdk", str(ctx.attr.min_sdk))
    arguments.add("--target-sdk", str(ctx.attr.target_sdk))
    arguments.add("--version-code", str(ctx.attr.version_code))
    arguments.add("--version-name", ctx.attr.version_name)
    arguments.add(library[0], format = "--library=%s")
    arguments.add(ctx.file._ndk.path, format = "--library=%s/sysroot/usr/lib/aarch64-linux-android/libc++_shared.so")
    arguments.add_all(ctx.files.java, format_each = "--java=%s")
    inputs = [ctx.file._jdk, ctx.file._build_tools, ctx.file._platform, ctx.file._ndk, ctx.file.manifest, library[0]] + ctx.files.java
    if ctx.attr.assets:
        assets = ctx.files.assets
        if len(assets) != 1:
            fail("assets is not one directory")
        arguments.add("--assets=" + assets[0].path)
        inputs.append(assets[0])
    ctx.actions.run(
        outputs = [out],
        inputs = inputs,
        executable = ctx.executable._apk,
        arguments = [arguments],
        mnemonic = "Apk",
        progress_message = "Assembling %{output}",
    )
    return [DefaultInfo(files = depset([out]))]

android_apk = rule(
    implementation = _android_apk_impl,
    attrs = {
        "application_id": attr.string(mandatory = True),
        "assets": attr.label(),
        "java": attr.label_list(allow_files = [".java"]),
        "label": attr.string(mandatory = True),
        "library": attr.label(),
        "manifest": attr.label(allow_single_file = True),
        "min_sdk": attr.int(),
        "package": attr.string(),
        "target_sdk": attr.int(),
        "version_code": attr.int(),
        "version_name": attr.string(),
        "_apk": attr.label(default = "//crates/bazel-tools:bazel-tools-bin", executable = True, cfg = "exec"),
        "_build_tools": attr.label(default = "//bazel/android:build-tools", allow_single_file = True, cfg = "exec"),
        "_jdk": attr.label(default = "//bazel/android:jdk", allow_single_file = True, cfg = "exec"),
        "_ndk": attr.label(default = "//bazel/tools:android-ndk", allow_single_file = True, cfg = "exec"),
        "_platform": attr.label(default = "//bazel/android:platform", allow_single_file = True, cfg = "exec"),
    },
    cfg = android_transition,
)

_SIGN = """
set -eu
jdk="$1" build_tools="$2" unsigned="$3" out="$4"
if [ -z "${ANDROID_DEBUG_KEYSTORE_BASE64:-}" ]; then
    echo 'BuildBuddy passed no ANDROID_DEBUG_KEYSTORE_BASE64: add it under the organization secrets.' >&2
    exit 1
fi
keystore="$(mktemp)"
printf '%s' "$ANDROID_DEBUG_KEYSTORE_BASE64" | base64 -d > "$keystore"
JAVA_HOME="$jdk" "$jdk/bin/java" -jar "$build_tools/lib/apksigner.jar" sign \\
    --ks "$keystore" --ks-pass pass:android --in "$unsigned" --out "$out"
rm -f "$keystore"
"""

# An APK signed on a worker with the keystore BuildBuddy keeps as a secret,
# which it passes only to actions on //bazel/platforms:android_signing: the
# target names //bazel/constraints:android_keystore in exec_compatible_with.
# The secret is not part of the action's key, so key_version is: bump it with
# the secret, or builds go on reusing APKs the old key signed.
def _signed_apk_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".apk")
    ctx.actions.run_shell(
        outputs = [out],
        inputs = [ctx.file._jdk, ctx.file._build_tools, ctx.file.apk],
        command = _SIGN,
        arguments = [
            ctx.file._jdk.path,
            ctx.file._build_tools.path,
            ctx.file.apk.path,
            out.path,
            "key-version-{}".format(ctx.attr.key_version),
        ],
        mnemonic = "ApkSign",
    )
    return [DefaultInfo(files = depset([out]))]

signed_apk = rule(
    implementation = _signed_apk_impl,
    attrs = {
        "apk": attr.label(allow_single_file = True),
        "key_version": attr.int(),
        "_build_tools": attr.label(default = "//bazel/android:build-tools", allow_single_file = True),
        "_jdk": attr.label(default = "//bazel/android:jdk", allow_single_file = True),
    },
    cfg = android_transition,
)
