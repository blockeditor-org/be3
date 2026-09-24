load("@prelude//cxx:cxx_toolchain_types.bzl", "CxxPlatformInfo", "CxxToolchainInfo", "LinkerInfo", "LinkerType", "RuntimeDependencyHandling")
load("@prelude//rust:rust_toolchain.bzl", "PanicRuntime", "RustToolchainInfo")
load("@prelude//toolchains:cxx.bzl", "CxxToolsInfo")
load("@root//buck/platforms:cross.bzl", "cross_triple", "per_cross_platform")

# A checked-in script, as a tool a toolchain can name. Every tool is an artifact
# rather than a name, so it is an input of the actions that run it and part of
# their key.
#
# A wrapper buck2 writes, and so marks executable, runs the script with /bin/sh:
# a Windows checkout has no executable bits, and a worker would refuse the
# uploaded script itself.
def _script_impl(ctx: AnalysisContext) -> list[Provider]:
    wrapper = ctx.actions.declare_output(ctx.label.name)
    ctx.actions.write(
        wrapper,
        [
            "#!/bin/sh",
            cmd_args(ctx.attrs.src, format = 'exec /bin/sh "$(dirname "$0")/{}" "$@"', relative_to = (wrapper, 1)),
        ],
        is_executable = True,
    )
    return [
        DefaultInfo(default_output = wrapper, other_outputs = [ctx.attrs.src]),
        RunInfo(args = cmd_args(wrapper, hidden = ctx.attrs.src)),
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
            archiver_type = ctx.attrs.archiver_type,
            asm_compiler = ctx.attrs.compiler[RunInfo].args,
            asm_compiler_type = "clang",
            compiler = ctx.attrs.compiler[RunInfo].args,
            compiler_type = "clang",
            cvtres_compiler = None,
            cxx_compiler = ctx.attrs.cxx_compiler[RunInfo].args,
            linker = (ctx.attrs.linker or ctx.attrs.cxx_compiler)[RunInfo].args,
            linker_type = LinkerType(ctx.attrs.linker_type),
            rc_compiler = None,
        ),
    ]

# prelude//toolchains/cxx/clang's tools, as artifacts. linker_type is "gnu" for
# ELF, "darwin" for ld64.lld, "windows" for lld-link.
host_cxx_tools = rule(
    attrs = {
        "archiver": attrs.exec_dep(providers = [RunInfo]),
        "archiver_type": attrs.string(default = "gnu"),
        "compiler": attrs.exec_dep(providers = [RunInfo]),
        "cxx_compiler": attrs.exec_dep(providers = [RunInfo]),
        "linker": attrs.option(attrs.exec_dep(providers = [RunInfo]), default = None),
        "linker_type": attrs.string(default = "gnu"),
    },
    impl = _host_cxx_tools_impl,
)

# The same for WebAssembly, linked by rust-lld: rustc hands a wasm link lld's own
# flags, which a clang driver does not take.
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

# prelude//toolchains:rust.bzl's system_rust_toolchain, with its tools as
# artifacts, so a different rustc is a different action key.
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

# A cxx toolchain whose links run on a worker, where the prelude's forces them
# local, and which puts a binary's shared libraries in a tree beside it with an
# $ORIGIN rpath, since a worker's container does not have them. Providers have
# no copy-with-changes, so the two that change are rebuilt from their fields.
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
