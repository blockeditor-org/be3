# The Rust standard library, as the prebuilt rlibs of the pinned rust-std component,
# for the Rust toolchain's explicit_sysroot_deps (advanced_unstable_linking: the
# prelude then links Rust through the cxx toolchain, and a library can be a shared
# library of its own). See buck/toolchains/BUCK.
#
# rust_std_libs is the one rule that links: every rlib of the standard library whole
# into one shared library (what libstd's own .so is, minus the crate hash in its
# name) and, for a static link, the rlibs as archives. rust_std_crate is one crate of
# it, for rustc's --extern and -L, which forwards rust_std_libs' link providers.

load("@prelude//cxx:cxx_context.bzl", "get_cxx_toolchain_info")
load("@prelude//cxx:cxx_toolchain_types.bzl", "CxxToolchainInfo", "PicBehavior")
load("@prelude//cxx:link.bzl", "cxx_link_shared_library")
load("@prelude//cxx:link_types.bzl", "link_options")
load("@prelude//linking:execution_preference.bzl", "LinkExecutionPreference")
load("@prelude//linking:link_groups.bzl", "LinkGroupLibInfo", "merge_link_group_lib_info")
load(
    "@prelude//linking:link_info.bzl",
    "Archive",
    "ArchiveLinkable",
    "LibOutputStyle",
    "LinkArgs",
    "LinkInfo",
    "LinkInfos",
    "LinkStrategy",
    "MergedLinkInfo",
    "SharedLibLinkable",
    "create_merged_link_info",
    "set_link_info_link_whole",
)
load(
    "@prelude//linking:linkable_graph.bzl",
    "LinkableGraph",
    "create_linkable_graph",
    "create_linkable_graph_node",
    "create_linkable_node",
)
load(
    "@prelude//linking:shared_libraries.bzl",
    "SharedLibraryInfo",
    "create_shared_libraries",
    "merge_shared_libraries",
)
load("@prelude//linking:types.bzl", "Linkage")
load("@prelude//rust:build_params.bzl", "MetadataKind")
load("@prelude//rust:crate_name.bzl", "CrateName")
load(
    "@prelude//rust:link_info.bzl",
    "RustArtifact",
    "RustExportedLinkDeps",
    "RustLinkInfo",
    "RustLinkStrategyInfo",
    "RustLinkableGraphs",
    "RustNativeLinkDeps",
    "TransitiveDeps",
)
load("@prelude//third-party:providers.bzl", "ThirdPartyBuildInfo", "third_party_build_info")
load("@prelude//unix:providers.bzl", "UnixEnv", "create_unix_env_info")

_SO = "libbe_rust_std.so"

# What the rlibs' #[link] attributes name for x86_64-unknown-linux-gnu: libc's
# c, m, rt, pthread, util, dl, and unwind's gcc_s.
_SYSTEM_LIBS = ["-lc", "-lm", "-lrt", "-lpthread", "-lutil", "-ldl", "-lgcc_s"]

RustStdInfo = provider(fields = {"metadata": dict[str, Artifact], "rlibs": dict[str, Artifact]})

def _lib_dir(ctx: AnalysisContext) -> Artifact:
    return ctx.attrs.component[DefaultInfo].default_outputs[0].project("lib/rustlib/{}/lib".format(ctx.attrs.triple))

def _rust_std_libs_impl(ctx: AnalysisContext) -> list[Provider]:
    cxx_toolchain = get_cxx_toolchain_info(ctx)
    linker_type = cxx_toolchain.linker_info.type
    # Each crate's rlib beside its rmeta, which holds the metadata the rlib's own
    # lacks: rustc finds one from the other.
    pairs = ctx.actions.symlinked_dir("pairs", {
        file: _lib_dir(ctx).project(file)
        for file in ctx.attrs.rlibs + [rlib.replace(".rlib", ".rmeta") for rlib in ctx.attrs.rlibs]
    })
    lib_dir = pairs
    rlibs = [lib_dir.project(name) for name in ctx.attrs.rlibs]

    archives = LinkInfo(
        name = "rust_std",
        linkables = [
            ArchiveLinkable(archive = Archive(artifact = rlib), linker_type = linker_type, link_whole = False)
            for rlib in rlibs
        ],
        post_flags = _SYSTEM_LIBS,
    )
    whole = LinkInfo(
        name = "rust_std_whole",
        linkables = [
            ArchiveLinkable(archive = Archive(artifact = rlib), linker_type = linker_type, link_whole = True)
            for rlib in rlibs
        ],
    )
    shim = ctx.attrs.shim[DefaultInfo].sub_targets["static-pic"][DefaultInfo].default_outputs[0]
    linked = cxx_link_shared_library(
        ctx = ctx,
        output = "shared/" + _SO,
        opts = link_options(
            links = [
                LinkArgs(flags = ["-Wl,-z,notext", "-Wl,--allow-multiple-definition"]),
                LinkArgs(infos = [whole]),
                LinkArgs(flags = _SYSTEM_LIBS),
            ],
            link_execution_preference = LinkExecutionPreference("any"),
            category_suffix = "rust_std_shared",
            allow_cache_upload = None,
        ),
        name = _SO,
    ).linked_object

    link_infos = {
        LibOutputStyle("archive"): LinkInfos(default = archives),
        LibOutputStyle("pic_archive"): LinkInfos(default = archives),
        LibOutputStyle("shared_lib"): LinkInfos(default = LinkInfo(
            name = "rust_std_shared",
            # The allocator entry points an executable's shim calls, which the shared
            # library cannot export (hidden, and a static link has std's own).
            linkables = [
                SharedLibLinkable(lib = linked.output),
                ArchiveLinkable(archive = Archive(artifact = shim), linker_type = linker_type, link_whole = True),
            ],
        )),
    }
    preferred_linkage = Linkage("any")
    merged = create_merged_link_info(
        ctx,
        PicBehavior("supported"),
        link_infos,
        preferred_linkage = preferred_linkage,
    )
    shared_libs = create_shared_libraries(ctx, {_SO: linked})
    shared_library_info = merge_shared_libraries(ctx.actions, shared_libs)
    graph = create_linkable_graph(
        ctx,
        node = create_linkable_graph_node(
            ctx,
            linkable_node = create_linkable_node(
                ctx = ctx,
                preferred_linkage = preferred_linkage,
                link_infos = link_infos,
                shared_libs = shared_libs,
                default_soname = _SO,
            ),
        ),
    )
    return [
        DefaultInfo(default_output = linked.output, sub_targets = {"pairs": [DefaultInfo(default_output = pairs)]}),
        RustStdInfo(
            metadata = {name: lib_dir.project(file) for name, file in ctx.attrs.crates.items()},
            rlibs = {name: lib_dir.project(file.replace(".rmeta", ".rlib")) for name, file in ctx.attrs.crates.items()},
        ),
        merged,
        shared_library_info,
        graph,
        merge_link_group_lib_info(),
        third_party_build_info(actions = ctx.actions),
        create_unix_env_info(
            actions = ctx.actions,
            env = UnixEnv(label = ctx.label, native_libs = [shared_libs]),
        ),
    ]

rust_std_libs = rule(
    impl = _rust_std_libs_impl,
    attrs = {
        "component": attrs.dep(),
        "crates": attrs.dict(attrs.string(), attrs.string()),
        "shim": attrs.dep(),
        "labels": attrs.list(attrs.string(), default = []),
        "rlibs": attrs.list(attrs.string()),
        "triple": attrs.string(),
        "_cxx_toolchain": attrs.default_only(attrs.toolchain_dep(default = "toolchains//:cxx", providers = [CxxToolchainInfo])),
    },
)

def _rust_std_crate_impl(ctx: AnalysisContext) -> list[Provider]:
    libs = ctx.attrs.libs
    metadata = libs[RustStdInfo].metadata
    crate = CrateName(simple = ctx.attrs.crate, dynamic = None)
    own = metadata[ctx.attrs.crate]

    # Every crate of the standard library is on rustc's -L, as this crate's
    # transitive dependencies: it loads the ones it names by their hashes.
    def everything(artifacts):
        return ctx.actions.tset(TransitiveDeps, children = [
            ctx.actions.tset(TransitiveDeps, value = RustArtifact(artifact = artifact, crate = CrateName(simple = name, dynamic = None)))
            for name, artifact in artifacts.items()
        ])

    transitive = {kind: everything(metadata) for kind in MetadataKind}

    # Linking an executable wants each crate's rlib beside its rmeta; --extern
    # names the rmeta, since an rlib's own metadata is a stub.
    transitive[MetadataKind("full")] = ctx.actions.tset(TransitiveDeps, children = [transitive[MetadataKind("full")], everything(libs[RustStdInfo].rlibs)])
    outputs = {kind: own for kind in MetadataKind}

    singleton = {kind: ctx.actions.tset(TransitiveDeps, value = RustArtifact(artifact = outputs[kind], crate = crate)) for kind in MetadataKind}
    strategies = {
        strategy: RustLinkStrategyInfo(
            outputs = outputs,
            singleton_tset = singleton,
            transitive_deps = transitive,
            transitive_proc_macro_deps = set(),
            rust_debug_info = None,
        )
        for strategy in LinkStrategy
    }
    return [
        DefaultInfo(),
        libs[MergedLinkInfo],
        libs[SharedLibraryInfo],
        libs[LinkableGraph],
        libs[LinkGroupLibInfo],
        libs[ThirdPartyBuildInfo],
        RustLinkInfo(
            crate = crate,
            strategies = strategies,
            native_link_deps = ctx.actions.tset(RustNativeLinkDeps, value = [(libs.label.configured_target(), libs[MergedLinkInfo])]),
            exported_link_deps = ctx.actions.tset(RustExportedLinkDeps),
            shared_libs = libs[SharedLibraryInfo],
            third_party_build_info = libs[ThirdPartyBuildInfo],
            linkable_graphs = ctx.actions.tset(RustLinkableGraphs, value = [libs[LinkableGraph]]),
        ),
    ]

rust_std_crate = rule(
    impl = _rust_std_crate_impl,
    attrs = {
        "crate": attrs.string(),
        "labels": attrs.list(attrs.string(), default = []),
        "libs": attrs.dep(providers = [RustStdInfo]),
    },
)
