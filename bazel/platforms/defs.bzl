# The platforms the workspace is built for. Each is a target platform in
# bazel/platforms/BUILD.bazel with a config_setting beside it, <name>_setting,
# which is what a select() between them is written with.

# name: (constraints, Rust triple)
PLATFORMS = {
    "android_arm64": (["@platforms//os:android", "@platforms//cpu:aarch64"], "aarch64-linux-android"),
    "linux_arm64": (["@platforms//os:linux", "@platforms//cpu:aarch64"], "aarch64-unknown-linux-gnu"),
    "linux_x86_64": (["@platforms//os:linux", "@platforms//cpu:x86_64"], "x86_64-unknown-linux-gnu"),
    "macos_arm64": (["@platforms//os:macos", "@platforms//cpu:aarch64"], "aarch64-apple-darwin"),
    "macos_x86_64": (["@platforms//os:macos", "@platforms//cpu:x86_64"], "x86_64-apple-darwin"),
    "wasi": (["@platforms//os:wasi", "@platforms//cpu:wasm32", "//bazel/constraints:app"], "wasm32-wasip1-threads"),
    "wasi_guest": (["@platforms//os:wasi", "@platforms//cpu:wasm32", "//bazel/constraints:guest"], "wasm32-wasip1-threads"),
    "wasm32": (["@platforms//os:none", "@platforms//cpu:wasm32"], "wasm32-unknown-unknown"),
    "windows_arm64": (["@platforms//os:windows", "@platforms//cpu:aarch64"], "aarch64-pc-windows-msvc"),
    "windows_x86_64": (["@platforms//os:windows", "@platforms//cpu:x86_64"], "x86_64-pc-windows-msvc"),
}
