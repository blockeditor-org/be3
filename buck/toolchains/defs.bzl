load("@prelude//cxx:cxx_toolchain_types.bzl", "LinkerType")
load("@prelude//rust:rust_toolchain.bzl", "PanicRuntime", "RustToolchainInfo")
load("@prelude//toolchains:cxx.bzl", "CxxToolsInfo")

# A tool this repository hands buck2 as a file rather than as a name.
#
# Everything about a toolchain that buck2 is given as a plain string ends up in
# the command line of every action that uses it, and a string that is a path is
# a different string on every machine - which makes the action a different
# action, and the shared cache useless. An artifact does not have that problem:
# buck2 writes it into the command line as a path relative to the repository
# root, which is the same everywhere.
#
# The files are the wrappers ./scripts/buck writes into buck/tools. Each one
# names the version of the tool it runs, so two machines with the same compiler
# agree on the bytes and share a cache entry, and two machines with different
# compilers do not - which is the half of this that keeps the cache honest
# rather than fast.
def _script_impl(ctx: AnalysisContext) -> list[Provider]:
    return [
        DefaultInfo(default_output = ctx.attrs.src),
        RunInfo(args = cmd_args(ctx.attrs.src)),
    ]

script = rule(
    attrs = {"src": attrs.source()},
    impl = _script_impl,
)

def _host_cxx_tools_impl(ctx: AnalysisContext) -> list[Provider]:
    return [
        DefaultInfo(),
        CxxToolsInfo(
            archiver = ctx.attrs.archiver,
            archiver_type = "gnu",
            asm_compiler = ctx.attrs.compiler,
            asm_compiler_type = "clang",
            compiler = ctx.attrs.compiler,
            compiler_type = "clang",
            cvtres_compiler = None,
            cxx_compiler = ctx.attrs.cxx_compiler,
            linker = ctx.attrs.cxx_compiler,
            linker_type = LinkerType("gnu"),
            rc_compiler = None,
        ),
    ]

# The same tools prelude//toolchains/cxx/clang:path_clang_tools names, as files.
host_cxx_tools = rule(
    attrs = {
        "archiver": attrs.source(),
        "compiler": attrs.source(),
        "cxx_compiler": attrs.source(),
    },
    impl = _host_cxx_tools_impl,
)

# prelude//toolchains:rust.bzl's system_rust_toolchain, with the three tools it
# names as strings taken as files instead, for the reason above. rustc is the
# one that matters: without it, an action that compiles Rust says nothing about
# which rustc compiled it, and two machines on different toolchains would share
# each other's results.
def _pinned_rust_toolchain_impl(ctx: AnalysisContext) -> list[Provider]:
    return [
        DefaultInfo(),
        RustToolchainInfo(
            allow_lints = ctx.attrs.allow_lints,
            clippy_driver = RunInfo(args = cmd_args(ctx.attrs.clippy_driver)),
            clippy_toml = None,
            compiler = RunInfo(args = cmd_args(ctx.attrs.compiler)),
            default_edition = ctx.attrs.default_edition,
            deny_lints = ctx.attrs.deny_lints,
            doctests = ctx.attrs.doctests,
            nightly_features = ctx.attrs.nightly_features,
            panic_runtime = PanicRuntime("unwind"),
            report_unused_deps = ctx.attrs.report_unused_deps,
            rustc_binary_flags = ctx.attrs.rustc_binary_flags,
            rustc_flags = ctx.attrs.rustc_flags,
            rustc_target_triple = ctx.attrs.rustc_target_triple,
            rustc_test_flags = ctx.attrs.rustc_test_flags,
            rustdoc = RunInfo(args = cmd_args(ctx.attrs.rustdoc)),
            rustdoc_flags = ctx.attrs.rustdoc_flags,
            warn_lints = ctx.attrs.warn_lints,
        ),
    ]

pinned_rust_toolchain = rule(
    attrs = {
        "allow_lints": attrs.list(attrs.string(), default = []),
        "clippy_driver": attrs.source(),
        "compiler": attrs.source(),
        "default_edition": attrs.option(attrs.string(), default = None),
        "deny_lints": attrs.list(attrs.string(), default = []),
        "doctests": attrs.bool(default = False),
        "nightly_features": attrs.bool(default = True),
        "report_unused_deps": attrs.bool(default = False),
        "rustc_binary_flags": attrs.list(attrs.arg(), default = []),
        "rustc_flags": attrs.list(attrs.arg(), default = []),
        "rustc_target_triple": attrs.string(),
        "rustc_test_flags": attrs.list(attrs.arg(), default = []),
        "rustdoc": attrs.source(),
        "rustdoc_flags": attrs.list(attrs.arg(), default = []),
        "warn_lints": attrs.list(attrs.string(), default = []),
    },
    impl = _pinned_rust_toolchain_impl,
    is_toolchain_rule = True,
)
