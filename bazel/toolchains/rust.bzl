# Rust, rust-toolchain.toml's, as one repository: the compiler, cargo, clippy
# and rustfmt for the workers, which are Linux on x86_64, and beside them the
# standard library of every target the workspace is built for, pinned by hash.
# Each target's toolchain names the one compiler and only its own standard
# library, so the compiler is fetched and unpacked once rather than once a
# target, and a new target's library changes no other target's actions.

_VERSION = "1.98.0"

_HOST = "x86_64-unknown-linux-gnu"

# component: (sha256, the directory in the archive)
_COMPONENTS = {
    "cargo-{}-{}".format(_VERSION, _HOST): ("2f512d170d3dd23e16ababcda32ee2e6d5172d861a7af1f504e0b1e270cafab9", "cargo"),
    "clippy-{}-{}".format(_VERSION, _HOST): ("646c6bd2450ea4c32a3c78ad7a3727294eea1f1519718a3f1487424021695ffa", "clippy-preview"),
    "rust-analyzer-{}-{}".format(_VERSION, _HOST): ("624e3415f1c85906a463b3bac05d6fa9e160124c42e1c2466bb5c24b7855162a", "rust-analyzer-preview"),
    "rust-src-{}".format(_VERSION): ("0e977492aed5ff137815bf9b4b796e8a85b80fb2367ed64e82d5c5839870f2bf", "rust-src"),
    "rustc-{}-{}".format(_VERSION, _HOST): ("0e37cb339f447fc44d6d781073bacacebfdc5612f2600e4c7e84c266f5f3aced", "rustc"),
    "rustfmt-{}-{}".format(_VERSION, _HOST): ("8922a03e68265a74a8040590e28b173fb5d9d49655bb6eacfd880fbd3bef0dc3", "rustfmt-preview"),
}

# triple: (sha256 of its standard library, stdlib link flags, binary, static
# library and dynamic library extensions)
TARGETS = {
    "aarch64-apple-darwin": ("48c05269ed36fb5f0a8438065156891fe59fcb868d90cfed9b7209540d54b2ce", ["-lSystem", "-lresolv"], "", ".a", ".dylib"),
    "aarch64-linux-android": ("e71a6405289f64b5edf00e255a32fe5e3d2e97d2d86324c279b5c1f3f20bdde6", ["-ldl", "-llog"], "", ".a", ".so"),
    "aarch64-pc-windows-msvc": ("df28cbf39697dfbfac0633814b56c1ca6f07fd32a70ab2074cb095f3871ae338", ["advapi32.lib", "ws2_32.lib", "userenv.lib", "Bcrypt.lib"], ".exe", ".lib", ".dll"),
    "aarch64-unknown-linux-gnu": ("a36f7ac98af20ef0ba6368aace0345efabbebaef7962eba99f47190fd256162d", ["-ldl", "-lpthread"], "", ".a", ".so"),
    "wasm32-unknown-unknown": ("3bb537c09555b96020a38a3a8810c38908b194eff3c2cb6184a45dda5c6828de", [], ".wasm", ".a", ".wasm"),
    "wasm32-wasip1-threads": ("56002ff8232f0e82d446267b6d84dc394007e4b090b8f84b17124615802407f3", [], ".wasm", ".a", ".wasm"),
    "x86_64-apple-darwin": ("8923fa9d0e0407b8a492e59d568e7aceff75949e1eee2e3e1b60e174373890cb", ["-lSystem", "-lresolv"], "", ".a", ".dylib"),
    "x86_64-pc-windows-msvc": ("81d123f834111ebeaf8b1eed0e93b0b66ea4bcf4cc1d3836d9a0314d4ea94a73", ["advapi32.lib", "ws2_32.lib", "userenv.lib", "Bcrypt.lib"], ".exe", ".lib", ".dll"),
    "x86_64-unknown-linux-gnu": ("f5022e6c95a5ad23cca2513dc8281200f585fa188de6370aa37b128a43f876a3", ["-ldl", "-lpthread"], "", ".a", ".so"),
}

_BUILD = """\
load("@rules_rust//rust:toolchain.bzl", "rust_analyzer_toolchain", "rust_stdlib_filegroup", "rust_toolchain", "rustfmt_toolchain")

package(default_visibility = ["//visibility:public"])

filegroup(
    name = "rustc",
    srcs = ["bin/rustc"],
)

filegroup(
    name = "rustc_lib",
    srcs = glob(
        [
            "lib/*.so*",
            "lib/rustlib/{host}/codegen-backends/*.so",
            "lib/rustlib/{host}/lib/*.so*",
            "lib/rustlib/{host}/lib/*.rmeta",
        ],
        allow_empty = True,
    ),
)

filegroup(
    name = "rustdoc",
    srcs = ["bin/rustdoc"],
)

filegroup(
    name = "cargo",
    srcs = ["bin/cargo"],
)

filegroup(
    name = "clippy_driver_bin",
    srcs = ["bin/clippy-driver"],
)

filegroup(
    name = "cargo_clippy_bin",
    srcs = ["bin/cargo-clippy"],
)

filegroup(
    name = "rustfmt_bin",
    srcs = ["bin/rustfmt"],
)

filegroup(
    name = "rust-lld",
    srcs = ["lib/rustlib/{host}/bin/rust-lld"],
    data = glob(
        ["lib/rustlib/{host}/bin/gcc-ld/*"],
        allow_empty = True,
    ),
)

filegroup(
    name = "rust_analyzer_bin",
    srcs = ["bin/rust-analyzer"],
)

filegroup(
    name = "proc_macro_srv",
    srcs = ["libexec/rust-analyzer-proc-macro-srv"],
)

rustfmt_toolchain(
    name = "rustfmt_toolchain",
    rustc = ":rustc",
    rustc_lib = ":rustc_lib",
    rustfmt = ":rustfmt_bin",
)

rust_analyzer_toolchain(
    name = "rust_analyzer_toolchain",
    proc_macro_srv = ":proc_macro_srv",
    rust_analyzer = ":rust_analyzer_bin",
    rustc = ":rustc",
    rustc_srcs = "//lib/rustlib/src:rustc_srcs",
    version = "{version}",
)
"""

_TARGET = """
rust_stdlib_filegroup(
    name = "rust_std-{triple}",
    srcs = glob(
        [
            "lib/rustlib/{triple}/lib/*.rlib",
            "lib/rustlib/{triple}/lib/*.rmeta",
            "lib/rustlib/{triple}/lib/*{dylib_ext}*",
            "lib/rustlib/{triple}/lib/*{staticlib_ext}",
            "lib/rustlib/{triple}/lib/self-contained/**",
        ],
        allow_empty = True,
    ),
)

rust_toolchain(
    name = "rust_toolchain-{triple}",
    binary_ext = "{binary_ext}",
    cargo = ":cargo",
    cargo_clippy = ":cargo_clippy_bin",
    channel = "stable",
    clippy_driver = ":clippy_driver_bin",
    default_edition = "2024",
    dylib_ext = "{dylib_ext}",
    exec_triple = "{host}",
    linker = ":rust-lld",
    linker_type = "direct",
    opt_level = {opt_level},
    rust_doc = ":rustdoc",
    rust_std = ":rust_std-{triple}",
    rustc = ":rustc",
    rustc_lib = ":rustc_lib",
    rustfmt = ":rustfmt_bin",
    staticlib_ext = "{staticlib_ext}",
    stdlib_linkflags = {stdlib_linkflags},
    strip_level = {strip_level},
    target_triple = "{triple}",
    version = "{version}",
)
"""

def _rust_repository_impl(rctx):
    archives = dict(_COMPONENTS)
    for triple, (sha256, _, _, _, _) in TARGETS.items():
        archives["rust-std-{}-{}".format(_VERSION, triple)] = (sha256, "rust-std-" + triple)
    for archive, (sha256, directory) in sorted(archives.items()):
        output = "lib/rustlib/src/rust" if directory == "rust-src" else ""
        strip = "{}/{}".format(archive, directory)
        if directory == "rust-src":
            strip = strip + "/lib/rustlib/src/rust"
        rctx.download_and_extract(
            url = "https://static.rust-lang.org/dist/{}.tar.xz".format(archive),
            sha256 = sha256,
            stripPrefix = strip,
            output = output,
        )
    rctx.file("lib/rustlib/src/BUILD.bazel", 'filegroup(\n    name = "rustc_srcs",\n    srcs = glob(["**/*"]),\n    visibility = ["//visibility:public"],\n)\n')
    content = [_BUILD.format(host = _HOST, version = _VERSION)]
    for triple, (_, linkflags, binary_ext, staticlib_ext, dylib_ext) in sorted(TARGETS.items()):
        wasm = triple.startswith("wasm32")

        # The wasm targets are cargo's plugin profile in a dev build (opt-level
        # 2, debug info stripped), since cranelift is far slower at an
        # unoptimised module.
        content.append(_TARGET.format(
            binary_ext = binary_ext,
            dylib_ext = dylib_ext,
            host = _HOST,
            opt_level = json.encode({"dbg": "0", "fastbuild": "2" if wasm else "0", "opt": "3"}),
            staticlib_ext = staticlib_ext,
            stdlib_linkflags = json.encode(linkflags),
            strip_level = json.encode({"dbg": "none", "fastbuild": "debuginfo" if wasm else "none", "opt": "debuginfo" if wasm else "none"}),
            triple = triple,
            version = _VERSION,
        ))
    rctx.file("BUILD.bazel", "\n".join(content))
    return rctx.repo_metadata(reproducible = True)

_rust_repository = repository_rule(implementation = _rust_repository_impl)

def _rust_impl(mctx):
    _rust_repository(name = "rust")
    return mctx.extension_metadata(reproducible = True)

rust = module_extension(implementation = _rust_impl)
