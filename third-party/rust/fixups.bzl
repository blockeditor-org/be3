# What buck/cargo/third_party.bzl is told about a third-party crate that it
# cannot work out from cargo's plans alone, by crate name. Features and
# dependencies are cargo's, per platform, so nothing here is about those unless
# it is a feature cargo would not have turned on. A crate's build script runs,
# as it does under cargo, unless build_script is False, and the libraries it
# links and the archives it compiles reach the link.
#
# Keys:
#   build_script        False to not run the crate's build script.
#   build_script_env    Environment for the build script, with buck2's macros.
#   rustc_flags         More flags for the library's compile.
#   features            platform: features to turn on as well.
#   overlay             path in the crate: a file in this package that
#                       replaces it.
#   deps                More dependencies for the library.

# A -sys crate whose build script asks pkg-config, directly or through
# system-deps, how to link the library it binds: answered from the sysroot the
# target is built against (buck/sysroot), and the libraries it names reach the
# link.
_PKG_CONFIG = {
    "build_script_env": {"PKG_CONFIG": "$(exe_target //buck/sysroot:pkg-config)"},
}

FIXUPS = {
    "alsa-sys": _PKG_CONFIG,
    "atk-sys": _PKG_CONFIG,
    "cairo-sys-rs": _PKG_CONFIG,
    # The build script runs bindgen over the SDK's CoreAudio headers. It asks
    # xcrun where the SDK is unless it is told, and bindgen loads libclang from
    # the path it is given; both come from buck/tools. libclang does not find
    # clang's own headers from where buck/tools puts it, so it is told where
    # they are.
    "coreaudio-sys": {
        "build_script_env": {
            "BINDGEN_EXTRA_CLANG_ARGS": "-resource-dir=$(location //buck/tools:llvm)/lib/clang/20",
            "COREAUDIO_SDK_PATH": "$(location //buck/tools:macos-sdk)",
            "LIBCLANG_PATH": "$(location //buck/tools:llvm)/lib",
        },
    },
    # The arm64 backend beside the host's own. The plugins beside an arm64 app
    # (Android's APK, the Macs, Windows and Linux on arm64) are compiled for its
    # wasmtime ahead of time, on the workers - crates/block-app's app and
    # android-assets - with the one wasmtime built for them, so it is this one
    # that has to know arm64.
    "cranelift-codegen": {"features": {"linux-x86_64": ["arm64"]}},
    # The build script compiles the FreeType it vendors unless pkg-config finds
    # one, and a plugin paints with the one it ships.
    "freetype-sys": {
        "build_script_env": {"FREETYPE2_NO_PKG_CONFIG": "1"},
        "deps": ["//third-party/system:stdc++"],
    },
    "gdk-pixbuf-sys": _PKG_CONFIG,
    "gdk-sys": _PKG_CONFIG,
    "gdkx11-sys": _PKG_CONFIG,
    "gio-sys": _PKG_CONFIG,
    "glib-sys": _PKG_CONFIG,
    "gobject-sys": _PKG_CONFIG,
    "gtk-sys": _PKG_CONFIG,
    # The same as freetype-sys, for HarfBuzz.
    "harfbuzz-sys": {
        "build_script_env": {"HARFBUZZ_NO_PKG_CONFIG": "1"},
        "deps": ["//third-party/system:stdc++"],
    },
    "javascriptcore-rs-sys": _PKG_CONFIG,
    # The build script lists the WebGL extensions for the library to include,
    # by absolute path into the directory it ran in. That directory is the
    # build script's own action's and is gone by the time the library
    # compiles, so the overlay's build script copies each one into OUT_DIR,
    # which the library's compile has, and includes it from there.
    "khronos_api": {"overlay": {"build.rs": "overlays/khronos_api/build.rs"}},
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
        "build_script_env": {"SDKROOT": "$(location //buck/tools:macos-sdk)"},
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
    # The build script asks pkg-config for gbm and compiles a probe against its
    # headers, with warnings as errors, to learn which of gbm's newer functions
    # the compositor can call. The compiler it is given carries the link's
    # flags as well, which a compile alone warns are unused.
    "smithay": {
        "build_script_env": {
            "CFLAGS": "-Wno-unused-command-line-argument",
            "PKG_CONFIG": "$(exe_target //buck/sysroot:pkg-config)",
        },
    },
    "soup3-sys": _PKG_CONFIG,
    "webkit2gtk-sys": _PKG_CONFIG,
    # Windows API imports as raw-dylib, which rustc turns into import stubs
    # itself, rather than through the import library windows_<arch>_msvc
    # vendors. That library is needed by every final link, and a build
    # script's link search path reaches only the compile of its own crate.
    "windows-targets": {"rustc_flags": ["--cfg=windows_raw_dylib"]},
}
