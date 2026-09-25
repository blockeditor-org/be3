# What bazel/cargo/extension.bzl is told about a third-party crate that it
# cannot work out from cargo's plans alone. Features and dependencies are
# cargo's, per platform, so nothing here is about those unless it is a feature
# cargo would not have turned on. A crate with a build script has it run, as
# cargo would, unless build_script is False.
#
# Keys:
#   build_script        False to not run the crate's build script.
#   build_script_env    Environment for the build script; $(execpath) names a
#                       label in build_script_data (built for the target) or
#                       build_script_tools (built for the worker).
#   build_script_data, build_script_tools
#   rustc_flags         More flags for the library's compile.
#   features            platform: features to turn on as well.
#   overlay             path in the crate: a file that replaces it.

# A -sys crate whose build script asks pkg-config, directly or through
# system-deps, how to link the library it binds: answered from the sysroot the
# target is built against (bazel/sysroot), and the libraries it names reach the
# link.
_PKG_CONFIG = {
    "build_script_data": ["@@//bazel/sysroot:pkg-config"],
    "build_script_env": {"PKG_CONFIG": "$(execpath @@//bazel/sysroot:pkg-config)"},
}

FIXUPS = {
    "alsa-sys": _PKG_CONFIG,
    "atk-sys": _PKG_CONFIG,
    "cairo-sys-rs": _PKG_CONFIG,
    # The build script runs bindgen over the SDK's CoreAudio headers. It asks
    # xcrun where the SDK is unless it is told, and bindgen loads libclang from
    # the path it is given; both come from bazel/tools. libclang does not find
    # clang's own headers from where bazel/tools puts it, so it is told where
    # they are.
    "coreaudio-sys": {
        "build_script_env": {
            "BINDGEN_EXTRA_CLANG_ARGS": "-resource-dir=$(execpath @@//bazel/tools:llvm)/lib/clang/20",
            "COREAUDIO_SDK_PATH": "$(execpath @@//bazel/tools:macos-sdk)",
            "LIBCLANG_PATH": "$(execpath @@//bazel/tools:llvm)/lib",
        },
        "build_script_tools": ["@@//bazel/tools:llvm", "@@//bazel/tools:macos-sdk"],
    },
    # The arm64 backend beside the host's own. The plugins beside an arm64 app
    # (Android's APK, the Macs, Windows and Linux on arm64) are compiled for its
    # wasmtime ahead of time, on the workers - crates/block-app's app and
    # android-assets - with the one wasmtime built for them, so it is this one
    # that has to know arm64.
    "cranelift-codegen": {"features": {"linux_x86_64": ["arm64"]}},
    "gdk-pixbuf-sys": _PKG_CONFIG,
    "gdk-sys": _PKG_CONFIG,
    "gdkx11-sys": _PKG_CONFIG,
    "gio-sys": _PKG_CONFIG,
    "glib-sys": _PKG_CONFIG,
    "gobject-sys": _PKG_CONFIG,
    "gtk-sys": _PKG_CONFIG,
    "javascriptcore-rs-sys": _PKG_CONFIG,
    # The build script lists the WebGL extensions for the library to include,
    # by absolute path into the directory it ran in. That directory is the
    # build script's own action's and is gone by the time the library
    # compiles, so the overlay's build script copies each one into OUT_DIR,
    # which the library's compile has, and includes it from there.
    "khronos_api": {"overlay": {"build.rs": "@@//third-party/rust/overlays:khronos_api/build.rs"}},
    "libseat-sys": _PKG_CONFIG,
    # The build script asks pkg-config for libudev, and then whether it has
    # udev_hwdb_new by compiling a program with a rustc it looks for on PATH,
    # which a worker does not have. So its answers are given here instead: the
    # sysroot's libudev, Ubuntu 24.04's, has the hwdb, and the crate links it.
    "libudev-sys": {
        "build_script": False,
        "rustc_flags": ["--cfg=hwdb", "-ludev"],
    },
    # The build script compiles one Objective-C file, which is what catches an
    # Objective-C exception on the way to Rust; cc-rs asks xcrun for the SDK
    # unless SDKROOT says.
    "objc2-exception-helper": {
        "build_script_env": {"SDKROOT": "$(execpath @@//bazel/tools:macos-sdk)"},
        "build_script_tools": ["@@//bazel/tools:macos-sdk"],
    },
    # The build script compiles Oboe against the NDK. PROFILE only names a
    # prebuilt it would otherwise fetch. cc-rs adds a --target without an API
    # level unless it sees one in CXXFLAGS.
    "oboe-sys": {
        "build_script_env": {
            "CXXFLAGS_aarch64_linux_android": "--target=aarch64-linux-android26",
            "PROFILE": "debug",
        },
    },
    "pango-sys": _PKG_CONFIG,
    "soup3-sys": _PKG_CONFIG,
    "webkit2gtk-sys": _PKG_CONFIG,
    # Windows API imports as raw-dylib, which rustc turns into import stubs
    # itself, rather than through the import library windows_<arch>_msvc
    # vendors.
    "windows-targets": {"rustc_flags": ["--cfg=windows_raw_dylib"]},
}
