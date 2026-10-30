load("@prelude//test/inject_test_run_info.bzl", "inject_test_run_info")
load("@root//buck/app:defs.bzl", "plugin_data", "plugin_types")
load(
    "@root//buck/cargo:defs.bzl",
    "RemoteTestInfo",
    "cargo_example",
    "cargo_library",
    "cargo_library_sources",
    "cargo_manifest_sources",
    "cargo_test_sources",
    "cargo_wasm_facts",
    "time_limited",
)
load("@root//buck/platforms:cross.bzl", "per_cross_platform")
load("@root//buck/platforms:profile.bzl", "PROFILE_REFS", "dev_only", "keep_profile")

# WebAssembly modules, depended on from targets that are not WebAssembly: the
# app and the native tests only load a game or a plugin as a module, so the
# module is a dependency in another configuration, which these transitions give
# it. A game's test says `env = {"GAME_WASM": "$(location :module)"}`.
def _wasm32_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.wasm32[PlatformInfo], refs)

wasm32_transition = transition(
    impl = _wasm32_transition_impl,
    refs = {"wasm32": "root//buck/platforms:wasm32"} | PROFILE_REFS,
)

# The plugins' wasi. The app's is the same triple, told apart by a constraint
# (buck/constraints/BUCK) so that each gets its own wgpu.
def _wasi_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.wasi_guest[PlatformInfo], refs)

wasi_transition = transition(
    impl = _wasi_transition_impl,
    refs = {"wasi_guest": "root//buck/platforms:wasi_guest"} | PROFILE_REFS,
)

def _wasi_app_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.wasi[PlatformInfo], refs)

wasi_app_transition = transition(
    impl = _wasi_app_transition_impl,
    refs = {"wasi": "root//buck/platforms:wasi"} | PROFILE_REFS,
)

# A crate on beui's HTML core (buck/constraints:html) in the browser.
def _wasi_html_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.wasi_html[PlatformInfo], refs)

wasi_html_transition = transition(
    impl = _wasi_html_transition_impl,
    refs = {"wasi_html": "root//buck/platforms:wasi_html"} | PROFILE_REFS,
)

# A cdylib's "shared" output is the .wasm; this names it as the module it is,
# which for a plugin is the entry point its manifest names. A release build runs
# wasm-opt over it: after rustc, -O2 takes a further tenth off a plugin's code
# and a third off a game, where -Oz takes a little more at twice the time. -g
# keeps the name section, which is what a guest's backtrace is read with.
def _wasm_module_impl(ctx: AnalysisContext) -> list[Provider]:
    shared = ctx.attrs.library[DefaultInfo].sub_targets["shared"][DefaultInfo].default_outputs[0]
    name = (ctx.attrs.module or ctx.attrs.name) + ".wasm"
    if not ctx.attrs.optimize:
        return [DefaultInfo(default_output = ctx.actions.copy_file(name, shared))]
    module = ctx.actions.declare_output(name)
    ctx.actions.run(
        cmd_args(
            ctx.attrs._wasm_opt[RunInfo],
            "-O2",
            "-g",
            "--detect-features",
            shared,
            "-o",
            module.as_output(),
        ),
        category = "wasm_opt",
    )
    return [DefaultInfo(default_output = module)]

def _module_rule(cfg):
    return rule(
        attrs = {
            "library": attrs.transition_dep(cfg = cfg),
            "module": attrs.option(attrs.string(), default = None),
            "optimize": attrs.default_only(
                attrs.bool(
                    default = select({
                        "DEFAULT": False,
                        "root//buck/constraints:release": True,
                    })
                )
            ),
            "_wasm_opt": attrs.exec_dep(default = "root//buck/tools:wasm-opt"),
        },
        impl = _wasm_module_impl,
    )

wasm32_module = _module_rule(wasm32_transition)

wasi_module = _module_rule(wasi_transition)

wasi_app_module = _module_rule(wasi_app_transition)

wasi_html_module = _module_rule(wasi_html_transition)

# rustc links a cdylib with --no-entry, which strips the symbols a host needs to
# lay out a thread's storage and leaves nothing to run libc's constructors; the
# host does both itself, so a guest exports them.
plugin_exports = [
    "-Clink-arg=--export=__heap_base",
    "-Clink-arg=--export=__tls_base",
    "-Clink-arg=--export=__tls_size",
    "-Clink-arg=--export=__tls_align",
    "-Clink-arg=--export=__wasm_init_tls",
    "-Clink-arg=--export=__wasm_call_ctors",
]

# block-app on the web: the same exports, which wasm-bindgen needs to prepare the
# module for threads, and wasi-libc's setjmp and no-exceptions libc++ for the C
# and C++ it links.
wasi_app_flags = plugin_exports + [
    "-Clink-arg=-L$(location root//third-party/wasi:sysroot)/lib/wasm32-wasip1-threads/noeh",
    "-Clink-arg=$(location root//third-party/wasi:sysroot)/lib/wasm32-wasip1-threads/libsetjmp.a",
]

# The plugin SDK (guides/plugin_sdk.md): every crate the plugins have in
# common, linked whole into one module that exports all of it, which each
# plugin is linked against and the host links it to when it loads. Its
# :module is what a plugin's identity names, so the plugins are relinked when
# it changes.
PLUGIN_SDK = "root//crates/block-plugin-sdk:module"

def plugin_sdk():
    facts = cargo_wasm_facts()
    native.rust_library(
        name = "wasm",
        crate = facts.crate,
        crate_root = facts.crate_root,
        deps = facts.deps,
        edition = facts.edition,
        env = facts.env,
        features = facts.features,
        linker_flags = ["--be3-sdk"],
        mapped_srcs = cargo_manifest_sources(),
        preferred_linkage = "shared",
        rustc_flags = plugin_exports,
        srcs = cargo_library_sources(),
        target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"],
    )
    wasi_module(
        name = "module",
        library = ":wasm",
        module = "sdk",
        visibility = ["PUBLIC"],
    )

# Libraries linked by the loader with their own thread-local blocks, the design
# plugins move to (plans/plugin_libraries.md; guides/plugin_sdk.md,
# "Thread-locals"). Only crates/tls-diamond is built this way so far.
#
# wasm_library is the package's crate, built as its library (the rlib other
# crates compile against), linked whole and exported from a module of its own
# through library.rs beside Cargo.toml, which only names the crate. With no
# libraries it is linked as the plugin SDK is, owning the memory, the stack and
# the allocator; with some it is position-independent, leaves out everything
# they link and reaches their thread-locals through GOT.tls. linked_plugin is a
# package's crate linked the same way, but not exported: a plugin.
def _needs(libraries):
    return ["--be3-needs=$(location {})".format(library) for library in libraries]

def wasm_library(module, libraries = [], rustc_flags = []):
    facts = cargo_wasm_facts()
    cargo_library(rustc_flags = rustc_flags)
    native.rust_library(
        name = "wasm",
        crate = facts.crate + "_library",
        crate_root = "library.rs",
        deps = [":" + facts.name],
        edition = facts.edition,
        linker_flags = _needs(libraries) + ["--be3-library"] if libraries else ["--be3-sdk"],
        preferred_linkage = "shared",
        rustc_flags = plugin_exports + rustc_flags,
        srcs = ["library.rs"],
        target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"],
    )
    wasi_module(
        name = "module",
        library = ":wasm",
        module = module,
        visibility = ["PUBLIC"],
    )

def linked_plugin(module, libraries, rustc_flags = []):
    facts = cargo_wasm_facts()
    native.rust_library(
        name = "wasm",
        crate = facts.crate,
        crate_root = facts.crate_root,
        deps = facts.deps,
        edition = facts.edition,
        env = facts.env,
        features = facts.features,
        linker_flags = _needs(libraries),
        mapped_srcs = cargo_manifest_sources(),
        preferred_linkage = "shared",
        rustc_flags = plugin_exports + rustc_flags,
        srcs = cargo_library_sources(),
        target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"],
    )
    wasi_module(
        name = "module",
        library = ":wasm",
        module = module,
        visibility = ["PUBLIC"],
    )

# How an editor's module is linked. libraries = None (the default) links it
# against the plugin SDK; libraries = [] links every crate it uses into it, a
# standalone plugin (guides/plugin_sdk.md, "The raw plugin interface"): a WASI
# reactor, whose _initialize (wasi-libc's crt1-reactor.o) sets up libc's main
# thread and runs the constructors, and which the host instantiates as it is.
# Naming libraries is plans/plugin_libraries.md's next step.
def _editor_link(libraries):
    if libraries == None:
        return ["--be3-plugin=$(location {})".format(PLUGIN_SDK)], []
    if libraries:
        fail("an editor is linked against the plugin SDK (libraries = None) or standalone (libraries = []); naming libraries is not built yet")
    return [], [
        "-Clink-arg=$(location root//third-party/wasi:sysroot)/lib/wasm32-wasip1-threads/crt1-reactor.o",
        "-Clink-arg=--export=_initialize",
    ]

# The package's editor linked as :<name>, its module named module.wasm: what
# editor() makes :module with, and a second build of the same editor linked
# another way, such as triangle's :standalone.
def editor_module(name, module, libraries = None, extra_deps = [], visibility = ["PUBLIC"]):
    facts = cargo_wasm_facts()
    linker_flags, rustc_flags = _editor_link(libraries)
    library = "wasm" if name == "module" else name + "-wasm"
    native.rust_library(
        name = library,
        crate = facts.crate,
        crate_root = facts.crate_root,
        deps = facts.deps + extra_deps,
        edition = facts.edition,
        env = facts.env,
        features = facts.features,
        linker_flags = linker_flags,
        mapped_srcs = cargo_manifest_sources(),
        preferred_linkage = "shared",
        rustc_flags = plugin_exports + rustc_flags + dev_only(["-Copt-level=0"]),
        srcs = cargo_library_sources() + ["manifest.json"],
        target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"],
        visibility = visibility,
    )
    wasi_module(
        name = name,
        library = ":" + library,
        module = module,
        visibility = visibility,
    )

# An editor: the guest cdylib, :wasm (wasm32 only; a plugin is all behind
# cfg(target_arch = "wasm32")), :module named after its manifest's entry point,
# :manifest, and its wasm :test. test_env is extra environment for compiling the
# tests, which is how a test names a module it loads with include_bytes!.
# extra_deps are what cargo cannot name, such as a C library the plugin links.
# A plugin is linked against the plugin SDK, or as libraries says
# (editor_module above). types is the plugin's type module
# (a wasm32_module, such as checklist's //crates/editors/checklist/types:module),
# which :types names for the app to stage beside :module.
#
# Outside a release build the plugin crate itself is compiled at opt-level 0,
# like its tests below, while its workspace dependencies are compiled at 1
# (buck/cargo/defs.bzl) and its third-party ones keep the plugin profile's 2.
# Most of a plugin's code is beui's generics instantiated anew, so a change
# under beui recompiles every plugin, and at opt-level 2 LLVM made that the
# longest part of ./scripts/verify: thirty plugins took 1,130 seconds of
# compiling at 2 and 270 at 0. The modules are about two and a half times the
# size and take half as long again to precompile, and the app tests ran no
# slower.
#
# An editor with examples/ is built for the host too (buck/cargo/buckify.sh
# plans it there), as the library named after its package, and each
# examples/NAME.rs is :example-NAME on the host, such as a benchmark that
# drives the editor through block-ui-test.
def editor(module, visibility = ["PUBLIC"], test_env = {}, data = {}, extra_deps = [], types = None, libraries = None):
    editor_module(
        name = "module",
        extra_deps = extra_deps,
        libraries = libraries,
        module = module,
        visibility = visibility,
    )
    native.export_file(
        name = "manifest",
        src = "manifest.json",
        visibility = visibility,
    )
    plugin_data(
        name = "data",
        files = data,
        visibility = visibility,
    )
    plugin_types(
        name = "types",
        module = types,
        visibility = visibility,
    )
    plugin_tests(
        env = test_env,
        exports = plugin_exports,
        extra_deps = extra_deps,
    )
    examples = native.glob(["examples/*.rs"])
    if examples:
        cargo_library(extra_srcs = ["manifest.json"])
        for root in examples:
            cargo_example(root.removeprefix("examples/").removesuffix(".rs"))

# A crate's tests compiled to wasm and run by plugin-test-runner, which gives
# the module a plugin's imports; an editor's, block-editor-plugin's and
# block-editor-beui's.
#
# The test crate itself is compiled at opt-level 0 while its dependencies are
# compiled as the plugin's are. Every module instantiates beui's generics anew, so a
# change to beui recompiles about thirty of them at once, and at opt-level 2
# LLVM is most of that: the step took twice as long, though the tests run
# faster and the modules are smaller to precompile.
def plugin_tests(exports = [], env = {}, extra_deps = []):
    facts = cargo_wasm_facts()
    native.rust_binary(
        name = "test-module",
        crate = facts.crate,
        crate_root = facts.crate_root,
        deps = facts.test_deps + extra_deps,
        edition = facts.edition,
        env = facts.env | env,
        features = facts.test_features,
        mapped_srcs = cargo_manifest_sources(),
        rustc_flags = exports + ["--test", "-Copt-level=0"],
        srcs = cargo_test_sources() + native.glob(["manifest.json"]),
        target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"],
    )
    wasi_test(
        name = "test",
        manifest = "Cargo.toml",
        module = ":test-module",
        paintings = native.glob(["snapshots/*.paint"]),
        precompile = per_cross_platform(True, lambda _: False),
        runner = "//crates/plugin-test-runner:bin",
    )
    plugin_test_run(
        name = "test-run",
        manifest = "Cargo.toml",
        module = ":test-module",
        paintings = native.glob(["snapshots/*.paint"]),
        runner = "//crates/plugin-test-runner:bin",
    )

# Compiled to wasm, a plugin's tests paint with the FreeType and HarfBuzz it
# ships, so the accepted paintings in the crate's snapshots/ stay put. The guest
# finds them from CARGO_MANIFEST_DIR, taken from the crate's real Cargo.toml
# rather than a staged copy, and plugin-test-runner opens that directory, and
# USED_PAINTINGS' and CHANGED_PAINTINGS', to it and nothing else.
#
# The module is compiled for wasmtime once, in an action, rather than by every
# test process - about forty seconds a module. It is compiled for the x86_64
# baseline, since the compile runs on a worker and the test here, and only for
# the host: elsewhere the runner compiles it when the test runs. --quick
# compiles it without Cranelift's optimizations, which took a quarter off the
# compile and left the tests' running time as it was; Cranelift's single-pass
# register allocator, which would take off more, ran a deep paint out of stack.
def _precompiled(ctx: AnalysisContext, module: Artifact) -> cmd_args:
    if not ctx.attrs.precompile:
        return cmd_args(module)
    runner = ctx.attrs.runner[RunInfo]
    artifact = ctx.actions.declare_output(module.basename.removesuffix(".wasm") + ".cwasm")
    ctx.actions.run(
        cmd_args(runner, "--precompile-to", "--target", "x86_64-unknown-linux-gnu", "--quick", artifact.as_output(), module),
        category = "wasm_precompile",
        identifier = module.basename,
    )
    return cmd_args("--precompiled", artifact, module)

def _wasi_test_impl(ctx: AnalysisContext) -> list[Provider]:
    module = ctx.attrs.module[DefaultInfo].default_outputs[0]
    command = cmd_args(ctx.attrs.runner[RunInfo], _precompiled(ctx, module))
    env = dict(ctx.attrs.env)
    env["CARGO_MANIFEST_DIR"] = cmd_args(ctx.attrs.manifest, parent = 1)
    remote_command, remote_env = _on_worker(ctx, command, env)
    remote = RemoteTestInfo(command = remote_command, env = remote_env, hidden = ctx.attrs.paintings)
    return (
        [remote]
        + inject_test_run_info(
            ctx,
            ExternalRunnerTestInfo(
                command = [command],
                env = env,
                # They run here and accept paintings, where the rest run on a
                # worker.
                labels = ctx.attrs.labels + ["plugin"],
                run_from_project_root = True,
                type = "rust",
                use_project_relative_paths = False,
            ),
        )
        + [DefaultInfo(default_output = module)]
    )

# A plugin test's command and environment on a worker, where it draws through
# lavapipe, as the renderer's tests do, with LD_LIBRARY_PATH carried as
# BE3_LD_LIBRARY_PATH for the same reason (library_path_test in
# buck/cargo/defs.bzl).
def _on_worker(ctx: AnalysisContext, command: cmd_args, env: dict) -> (cmd_args, dict):
    sysroot = ctx.attrs._sysroot[DefaultInfo].default_outputs[0]
    return cmd_args("sh", "-c", 'LD_LIBRARY_PATH="$BE3_LD_LIBRARY_PATH" && export LD_LIBRARY_PATH && exec "$@"', "sh", command), env | {
        "BE3_LD_LIBRARY_PATH": cmd_args(sysroot, format = "{}/usr/lib/x86_64-linux-gnu"),
        "VK_ICD_FILENAMES": ctx.attrs._lavapipe,
    }

# What a plugin test runs, for buck2 test and for ./scripts/verify alike.
_plugin_test_attrs = {
    "manifest": attrs.source(),
    "module": attrs.transition_dep(cfg = wasi_transition),
    "paintings": attrs.list(attrs.source(), default = []),
    "precompile": attrs.bool(default = True),
    "_lavapipe": attrs.source(default = "root//buck/sysroot:lavapipe_icd.json"),
    "_sysroot": attrs.dep(default = "root//buck/sysroot:amd64-test"),
}

wasi_test = rule(
    attrs = _plugin_test_attrs
    | {
        "env": attrs.dict(default = {}, key = attrs.string(), value = attrs.arg()),
        "labels": attrs.list(attrs.string(), default = []),
        "runner": attrs.dep(providers = [RunInfo]),
        "_inject_test_env": attrs.default_only(attrs.dep(default = "prelude//test/tools:inject_test_env")),
    },
    impl = _wasi_test_impl,
)

# The plugin tests as an action, which is how ./scripts/verify runs them: on a
# worker, and answered from the cache when nothing they read has changed, which
# a test never is. They read the accepted paintings from the crate's
# snapshots/, and no other crate's, so accepting a painting leaves every other
# crate's tests in the cache. They accept every painting, writing the ones that
# changed or are new to changed/ and, for each accepted painting they compared,
# the SHA-256 of what they read to used/ (that of nothing when there was none),
# so that the paintings themselves are never downloaded; ./scripts/verify
# copies into the crate's snapshots/ the paintings in changed/ whose accepted
# painting still has that hash, or under --check fails on them.
def _plugin_test_run_impl(ctx: AnalysisContext) -> list[Provider]:
    module = ctx.attrs.module[DefaultInfo].default_outputs[0]
    paintings = ctx.actions.declare_output("paintings", dir = True, has_content_based_path = False)
    env = {"CARGO_MANIFEST_DIR": cmd_args(ctx.attrs.manifest, parent = 1), "UPDATE_SNAPSHOTS": "1"}
    command, env = _on_worker(ctx, cmd_args(ctx.attrs.runner[RunInfo], _precompiled(ctx, module)), env)
    ctx.actions.run(
        cmd_args(
            "sh",
            "-c",
            'mkdir -p "$1/used" "$1/changed" && USED_PAINTINGS="$1/used" CHANGED_PAINTINGS="$1/changed" && export USED_PAINTINGS CHANGED_PAINTINGS && shift && exec "$@"',
            "sh",
            paintings.as_output(),
            time_limited(command),
            hidden = ctx.attrs.paintings,
        ),
        category = "plugin_test",
        env = env,
    )
    return [DefaultInfo(default_output = paintings)]

plugin_test_run = rule(
    attrs = _plugin_test_attrs
    | {
        "runner": attrs.exec_dep(providers = [RunInfo]),
    },
    impl = _plugin_test_run_impl,
)
