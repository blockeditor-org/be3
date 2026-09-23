load("@prelude//cxx:cxx_toolchain_types.bzl", "CxxPlatformInfo", "CxxToolchainInfo", "LinkerInfo", "LinkerType", "RuntimeDependencyHandling")
load("@prelude//rust:rust_toolchain.bzl", "PanicRuntime", "RustToolchainInfo")
load("@prelude//toolchains:cxx.bzl", "CxxToolsInfo")
load("@root//buck/platforms:cross.bzl", "cross_triple", "per_cross_platform")

# A checked-in script, as a tool a toolchain can name.
#
# Every tool reaches buck2 as an artifact rather than as a name. Anything a
# toolchain gives buck2 as a plain string ends up in the command line of every
# action that uses it and is resolved wherever the action runs; an artifact is
# written as a path relative to the repository root and is an input of the
# action, so the worker has exactly the file this repository says it should.
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
            archiver = ctx.attrs.archiver[RunInfo].args,
            archiver_type = "gnu",
            asm_compiler = ctx.attrs.compiler[RunInfo].args,
            asm_compiler_type = "clang",
            compiler = ctx.attrs.compiler[RunInfo].args,
            compiler_type = "clang",
            cvtres_compiler = None,
            cxx_compiler = ctx.attrs.cxx_compiler[RunInfo].args,
            linker = ctx.attrs.cxx_compiler[RunInfo].args,
            linker_type = LinkerType(ctx.attrs.linker_type),
            rc_compiler = None,
        ),
    ]

# The same tools prelude//toolchains/cxx/clang:path_clang_tools names, as
# artifacts. The linker is the C++ driver, and linker_type is what the prelude
# shapes its link flags for: "gnu" for an ELF link, "darwin" for ld64.lld.
host_cxx_tools = rule(
    attrs = {
        "archiver": attrs.exec_dep(providers = [RunInfo]),
        "compiler": attrs.exec_dep(providers = [RunInfo]),
        "cxx_compiler": attrs.exec_dep(providers = [RunInfo]),
        "linker_type": attrs.string(default = "gnu"),
    },
    impl = _host_cxx_tools_impl,
)

# The same, for WebAssembly.
#
# What differs is the linker. rustc emits lld's own flags for a wasm link -
# -flavor wasm, --export, --no-entry - and hands them to whatever the cxx
# toolchain calls a linker, which for the host is a clang driver that has never
# heard of them. rust-lld is the linker rustc would have used itself, and it
# ships in the same toolchain, so the two always agree on them.
def _wasm_cxx_tools_impl(ctx: AnalysisContext) -> list[Provider]:
    return [
        DefaultInfo(),
        CxxToolsInfo(
            archiver = ctx.attrs.archiver[RunInfo].args,
            archiver_type = "gnu",
            asm_compiler = ctx.attrs.compiler[RunInfo].args,
            asm_compiler_type = "clang",
            compiler = ctx.attrs.compiler[RunInfo].args,
            compiler_type = "clang",
            cvtres_compiler = None,
            cxx_compiler = ctx.attrs.cxx_compiler[RunInfo].args,
            linker = ctx.attrs.linker[RunInfo].args,
            linker_type = LinkerType("wasm"),
            rc_compiler = None,
        ),
    ]

wasm_cxx_tools = rule(
    attrs = {
        "archiver": attrs.exec_dep(providers = [RunInfo]),
        "compiler": attrs.exec_dep(providers = [RunInfo]),
        "cxx_compiler": attrs.exec_dep(providers = [RunInfo]),
        "linker": attrs.exec_dep(providers = [RunInfo]),
    },
    impl = _wasm_cxx_tools_impl,
)

# prelude//toolchains:rust.bzl's system_rust_toolchain, with the three tools it
# names as strings taken as artifacts instead, for the reason above. rustc is
# the one that matters: as an artifact it is part of every Rust action's key,
# so a different rustc is a different action rather than a cache hit on
# another compiler's output.
def _pinned_rust_toolchain_impl(ctx: AnalysisContext) -> list[Provider]:
    return [
        DefaultInfo(),
        RustToolchainInfo(
            allow_lints = ctx.attrs.allow_lints,
            clippy_driver = ctx.attrs.clippy_driver[RunInfo],
            clippy_toml = ctx.attrs.clippy_toml,
            compiler = ctx.attrs.compiler[RunInfo],
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
            rustdoc = ctx.attrs.rustdoc[RunInfo],
            rustdoc_flags = ctx.attrs.rustdoc_flags,
            warn_lints = ctx.attrs.warn_lints,
        ),
    ]

pinned_rust_toolchain = rule(
    attrs = {
        "allow_lints": attrs.list(attrs.string(), default = []),
        "clippy_driver": attrs.exec_dep(providers = [RunInfo]),
        "clippy_toml": attrs.source(),
        "compiler": attrs.exec_dep(providers = [RunInfo]),
        "default_edition": attrs.option(attrs.string(), default = None),
        "deny_lints": attrs.list(attrs.string(), default = []),
        "doctests": attrs.bool(default = False),
        "nightly_features": attrs.bool(default = True),
        "report_unused_deps": attrs.bool(default = False),
        "rustc_binary_flags": attrs.list(attrs.arg(), default = []),
        "rustc_flags": attrs.list(attrs.arg(), default = []),
        "rustc_target_triple": attrs.string(),
        "rustc_test_flags": attrs.list(attrs.arg(), default = []),
        "rustdoc": attrs.exec_dep(providers = [RunInfo]),
        "rustdoc_flags": attrs.list(attrs.arg(), default = []),
        "warn_lints": attrs.list(attrs.string(), default = []),
    },
    impl = _pinned_rust_toolchain_impl,
    is_toolchain_rule = True,
)

# A cxx toolchain whose links run on a remote worker.
#
# prelude//toolchains:cxx.bzl hard-codes every link and archive to run locally,
# which is the right call for a toolchain that is whatever is on PATH: a worker
# would not have it. This one is buck/tools', which a worker has, so the
# preference would only cost a download - the whole compiler and every rlib a
# binary links, onto a machine that then runs a linker it may not be able to
# load. This passes the toolchain through with the three preferences cleared.
#
# It also puts a binary's shared libraries beside it. The demo toolchain
# leaves them wherever they were built and trusts the machine that runs the
# binary to find them, which a worker cannot: the one shared library the build
# links, ALSA's, is not in its container. With symlink handling the binary gets
# a tree of them next to it, an $ORIGIN rpath into that tree, and the tree as
# an input of whatever runs it.
#
# Providers have no copy-with-changes, so the two that change are rebuilt from
# their own fields.
def _replace(constructor, value, **changes):
    fields = {name: getattr(value, name) for name in dir(value)}
    fields.update(changes)
    return constructor(**fields)

def _remote_linking_cxx_toolchain_impl(ctx: AnalysisContext) -> list[Provider]:
    toolchain = ctx.attrs.toolchain
    info = toolchain[CxxToolchainInfo]
    linker_info = _replace(
        LinkerInfo,
        info.linker_info,
        archive_objects_locally = False,
        link_binaries_locally = False,
        link_libraries_locally = False,
    )
    return [
        DefaultInfo(),
        _replace(
            CxxToolchainInfo,
            info,
            linker_info = linker_info,
            runtime_dependency_handling = RuntimeDependencyHandling("symlink"),
        ),
        toolchain[CxxPlatformInfo],
    ]

remote_linking_cxx_toolchain = rule(
    attrs = {
        "toolchain": attrs.toolchain_dep(providers = [CxxToolchainInfo]),
    },
    impl = _remote_linking_cxx_toolchain_impl,
    is_toolchain_rule = True,
)

# One of buck/tools' Rust programs, from the sysroot the target is built with:
# the host and wasm targets share one, and a cross-compiled target has its own,
# which buck/tools/BUCK says why.
def cross_tool(tools: str, program: str):
    return per_cross_platform(
        "{}:{}".format(tools, program),
        lambda name: "{}:{}-{}".format(tools, program, cross_triple(name)),
    )
