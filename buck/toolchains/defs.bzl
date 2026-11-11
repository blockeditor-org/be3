load("@prelude//cxx:cxx_toolchain_types.bzl", "CxxPlatformInfo", "CxxToolchainInfo", "LinkerInfo", "LinkerType", "RuntimeDependencyHandling")
load("@prelude//rust:link_info.bzl", "RustLinkInfo")
load("@prelude//rust:rust_toolchain.bzl", "PanicRuntime", "RustExplicitSysrootDeps", "RustToolchainInfo")
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
    std = {name: dep for name, dep in ctx.attrs.std.items() if dep != None}
    info = RustToolchainInfo(
        advanced_unstable_linking = ctx.attrs.advanced_unstable_linking,
        explicit_sysroot_deps = RustExplicitSysrootDeps(
            core = std["core"],
            proc_macro = std["proc_macro"],
            std = std["std"],
            panic_unwind = std["panic_unwind"],
            panic_abort = std["panic_abort"],
            others = [dep for name, dep in std.items() if name not in ["core", "proc_macro", "std", "panic_unwind", "panic_abort"]],
        ) if std else None,
        extra_rustc_flags = ctx.attrs.extra_rustc_flags,
        linker_flags = ctx.attrs.linker_flags,
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
    )
    _check_fields("RustToolchainInfo", info, _RUST_TOOLCHAIN_FIELDS)
    return [DefaultInfo(), info]

pinned_rust_toolchain = rule(
    attrs = {
        "advanced_unstable_linking": attrs.bool(default = False),
        "allow_lints": attrs.list(attrs.string(), default = []),
        "extra_rustc_flags": attrs.list(attrs.arg(), default = []),
        "linker_flags": attrs.list(attrs.arg(), default = []),
        "std": attrs.dict(attrs.string(), attrs.option(attrs.dep(providers = [RustLinkInfo])), default = {}),
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

# The fields of the prelude's providers the rules here were written against.
# pinned_rust_toolchain sets some by name and leaves the rest to defaults, and
# remote_linking_cxx_toolchain copies them all and changes a few, so a buck2
# release that adds, drops or renames one stops every build here at analysis,
# with what changed, rather than changing what builds do unseen.
_CXX_TOOLCHAIN_FIELDS = [
    "as_compiler_info",
    "asm_compiler_info",
    "binary_utilities_info",
    "bolt_enabled",
    "c_compiler_info",
    "cell_to_path_prefix_map",
    "clang_llvm_statistics",
    "clang_remarks",
    "clang_trace",
    "compiler_flavor_flags",
    "cpp_dep_tracking_mode",
    "cuda_compiler_info",
    "cuda_dep_tracking_mode",
    "cvtres_compiler_info",
    "cxx_compiler_info",
    "cxx_error_handler",
    "default_deps",
    "dumpbin_toolchain_path",
    "gcno_files",
    "header_mode",
    "headers_as_raw_headers_mode",
    "hip_compiler_info",
    "hip_debug_extract",
    "hip_device_debug_extract",
    "hip_gpu_archs",
    "internal_tools",
    "libclang",
    "linker_info",
    "lipo",
    "llvm_cgdata",
    "llvm_link",
    "materialize_external_debug_info",
    "minimum_os_version",
    "objc_compiler_info",
    "objcxx_compiler_info",
    "object_format",
    "pass_plugin",
    "pic_behavior",
    "raw_headers_as_headers_mode",
    "rc_compiler_info",
    "remap_cwd",
    "runtime_dependency_handling",
    "split_debug_mode",
    "strip_flags_info",
    "supported_compile_flavors",
    "target_stats_tools",
    "use_dep_files",
    "use_distributed_thinlto",
]

_LINKER_FIELDS = [
    "archive_contents",
    "archive_objects_locally",
    "archive_symbol_table",
    "archiver",
    "archiver_flags",
    "archiver_reads_inputs",
    "archiver_supports_argfiles",
    "archiver_type",
    "binary_extension",
    "binary_linker_flags",
    "dist_thin_lto_codegen_flags",
    "executable_linker_flags",
    "extra_outputs",
    "force_full_hybrid_if_capable",
    "generate_gc_sections",
    "generate_linker_maps",
    "incremental_linking",
    "independent_shlib_interface_linker_flags",
    "is_pdb_generated",
    "link_binaries_locally",
    "link_libraries_locally",
    "link_metadata_flag",
    "link_ordering",
    "link_style",
    "link_weight",
    "linker",
    "linker_flags",
    "lto_mode",
    "mk_shlib_intf",
    "object_file_extension",
    "post_linker_flags",
    "push_pop_state_flags",
    "requires_archives",
    "requires_objects",
    "runtime_library_files",
    "sanitizer_runtime_enabled",
    "sanitizer_runtime_files",
    "shared_dep_runtime_ld_flags",
    "shared_library_name_default_prefix",
    "shared_library_name_format",
    "shared_library_versioned_name_format",
    "shlib_interfaces",
    "static_dep_runtime_ld_flags",
    "static_library_extension",
    "static_pic_dep_runtime_ld_flags",
    "supports_content_based_paths_for_archiving",
    "supports_distributed_thinlto",
    "supports_shared_libraries",
    "thin_lto_double_codegen_enabled",
    "thin_lto_premerger_enabled",
    "type",
    "use_archiver_flags",
]

_RUST_TOOLCHAIN_FIELDS = [
    "advanced_unstable_linking",
    "allow_lints",
    "clippy_driver",
    "clippy_toml",
    "compiler",
    "configuration_hash",
    "default_edition",
    "deny_lints",
    "deny_on_check_lints",
    "dist_thin_lto_codegen_flags",
    "doctests",
    "explicit_sysroot_deps",
    "extra_rustc_flags",
    "linker_flags",
    "llvm_lines_tool",
    "make_trace_upload",
    "measureme_crox",
    "nightly_features",
    "panic_runtime",
    "pgo_generate_dir",
    "pgo_profile",
    "remarks",
    "report_unused_deps",
    "restricted_rustc_flags",
    "rust_error_handler",
    "rust_target_path",
    "rustc_binary_flags",
    "rustc_check_flags",
    "rustc_coverage_flags",
    "rustc_env",
    "rustc_flags",
    "rustc_target_triple",
    "rustc_test_flags",
    "rustdoc",
    "rustdoc_env",
    "rustdoc_flags",
    "sanitizer",
    "strip_dwo_from_rlibs",
    "sysroot_path",
    "warn_lints",
]

def _check_fields(provider: str, value, expected: list[str]):
    fields = sorted(dir(value))
    if fields != expected:
        fail(
            "buck2's prelude changed {}: buck/toolchains/defs.bzl was written against\n  {}\nand it now has\n  {}\nCheck what each field added or dropped does to the toolchains, then update the list.".format(
                provider, expected, fields
            )
        )

# A cxx toolchain whose links run on a worker, where the prelude's forces them
# local, and which puts a binary's shared libraries in a tree beside it with an
# $ORIGIN rpath, since a worker's container does not have them. Providers have
# no copy-with-changes, so the two that change are rebuilt from their fields.
def _replace(constructor, value, **changes):
    fields = {name: getattr(value, name) for name in dir(value)}
    fields.update(changes)
    return constructor(**fields)

#
# The prelude's cxx_toolchain_override does the same for the locality flags, but
# not for runtime_dependency_handling, and it cannot take this base: it builds
# Objective-C compiler infos from the base's, which cxx_tools_info_toolchain
# leaves None, and its macro selects on config//linker/constraints, which the
# bundled prelude does not have.
def _remote_linking_cxx_toolchain_impl(ctx: AnalysisContext) -> list[Provider]:
    toolchain = ctx.attrs.toolchain
    info = toolchain[CxxToolchainInfo]
    _check_fields("CxxToolchainInfo", info, _CXX_TOOLCHAIN_FIELDS)
    _check_fields("LinkerInfo", info.linker_info, _LINKER_FIELDS)
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
