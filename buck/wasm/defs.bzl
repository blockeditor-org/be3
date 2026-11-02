load("@prelude//test/inject_test_run_info.bzl", "inject_test_run_info")
load("@root//buck/program:defs.bzl", "app_data", "app_types")
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
load("@root//buck/platforms:profile.bzl", "PROFILE_REFS", "RELEASE_SYMBOL_FLAGS", "dev_only", "keep_profile")
load("@root//buck/wasm:libraries.bzl", "WasmLibrariesInfo")

# WebAssembly modules, depended on from targets that are not WebAssembly: the
# app and the native tests only load a game or an app as a module, so the
# module is a dependency in another configuration, which these transitions give
# it. A game's test says `env = {"GAME_WASM": "$(location :module)"}`.
def _wasm32_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.wasm32[PlatformInfo], refs)

wasm32_transition = transition(
    impl = _wasm32_transition_impl,
    refs = {"wasm32": "root//buck/platforms:wasm32"} | PROFILE_REFS,
)

# The apps' wasi. The app's is the same triple, told apart by a constraint
# (buck/constraints/BUCK) so that each gets its own wgpu.
def _wasi_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.wasi_guest[PlatformInfo], refs)

wasi_transition = transition(
    impl = _wasi_transition_impl,
    refs = {"wasi_guest": "root//buck/platforms:wasi_guest"} | PROFILE_REFS,
)

def _wasi_program_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.wasi[PlatformInfo], refs)

wasi_program_transition = transition(
    impl = _wasi_program_transition_impl,
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
# which for an app is the entry point its manifest names. A release build runs
# wasm-opt over it: after rustc, -O2 takes a further tenth off an app's code
# and a third off a game, where -Oz takes a little more at twice the time. -g
# keeps the name section, which is what a guest's backtrace is read with.
def _wasm_module_impl(ctx: AnalysisContext) -> list[Provider]:
    shared = ctx.attrs.library[DefaultInfo].sub_targets["shared"][DefaultInfo].default_outputs[0]
    name = (ctx.attrs.module or ctx.attrs.name) + ".wasm"
    closure = []
    for library in ctx.attrs.libraries:
        for module in library[WasmLibrariesInfo].closure + [library[DefaultInfo].default_outputs[0]]:
            if module not in closure:
                closure.append(module)
    libraries = WasmLibrariesInfo(closure = closure)
    if not ctx.attrs.optimize:
        return [DefaultInfo(default_output = ctx.actions.copy_file(name, shared)), libraries]
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
    return [DefaultInfo(default_output = module), libraries]

def _module_rule(cfg):
    return rule(
        attrs = {
            # The libraries it is linked against (wasm_library), whose
            # closure it gives as WasmLibrariesInfo.
            "libraries": attrs.list(attrs.dep(providers = [WasmLibrariesInfo]), default = []),
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

wasi_program_module = _module_rule(wasi_program_transition)

wasi_html_module = _module_rule(wasi_html_transition)

# rustc links a cdylib with --no-entry, which strips the symbols a host needs to
# lay out a thread's storage and leaves nothing to run libc's constructors; the
# host does both itself, so a guest exports them.
guest_exports = [
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
wasi_program_flags = guest_exports + [
    "-Clink-arg=-L$(location root//third-party/wasi:sysroot)/lib/wasm32-wasip1-threads/noeh",
    "-Clink-arg=$(location root//third-party/wasi:sysroot)/lib/wasm32-wasip1-threads/libsetjmp.a",
]

# The libraries every editor is linked against unless it names its own
# (guides/app_libraries.md): base, the root that holds std, wasi-libc, wgpu and
# be-sdk, and beui above it.
BASE_LIBRARY = "root//crates/be-sdk-library-base:module"

DEFAULT_LIBRARIES = [BASE_LIBRARY, "root//crates/be-sdk-library-beui:module"]

# What makes a module the root of every closure: wasi-libc's crt1-reactor.o,
# whose _initialize sets up libc's main thread and runs the constructors. The
# root and a standalone app are each a WASI reactor linked at fixed
# addresses, which the host instantiates as it is.
_reactor = [
    "-Clink-arg=$(location root//third-party/wasi:sysroot)/lib/wasm32-wasip1-threads/crt1-reactor.o",
    "-Clink-arg=--export=_initialize",
]

def _needs(libraries):
    return ["--be3-needs=$(location {})".format(library) for library in libraries]

# A library apps are linked against (guides/app_libraries.md): the package's
# crate linked whole into a module of its own that exports everything, which
# the host links each app that names it to when it loads. libraries names
# every library under it, the whole closure; with none it is the root, linked
# at fixed addresses with std and wasi-libc, and with some it is
# position-independent, leaves out everything they link and reaches their
# thread-locals through GOT.tls. A package whose crate other crates depend on,
# such as crates/be-tool-tls-diamond-base, has a library.rs beside its Cargo.toml that only
# names the crate, and that is what is linked, so the module holds the very rlib
# its dependents compile against.
def wasm_library(module, libraries = [], rustc_flags = []):
    facts = cargo_wasm_facts()
    linker_flags = _needs(libraries) + ["--be3-library=" + module]
    rustc_flags = guest_exports + rustc_flags + ([] if libraries else _reactor) + RELEASE_SYMBOL_FLAGS
    if native.glob(["library.rs"]):
        cargo_library(rustc_flags = rustc_flags)
        native.rust_library(
            name = "wasm",
            crate = facts.crate + "_library",
            crate_root = "library.rs",
            deps = [":" + facts.name],
            edition = facts.edition,
            linker_flags = linker_flags,
            preferred_linkage = "shared",
            rustc_flags = rustc_flags,
            srcs = ["library.rs"],
            target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"],
        )
    else:
        native.rust_library(
            name = "wasm",
            crate = facts.crate,
            crate_root = facts.crate_root,
            deps = facts.deps,
            edition = facts.edition,
            env = facts.env,
            features = facts.features,
            linker_flags = linker_flags,
            mapped_srcs = cargo_manifest_sources(),
            preferred_linkage = "shared",
            rustc_flags = rustc_flags,
            srcs = cargo_library_sources(),
            target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"],
        )
    wasi_module(
        name = "module",
        libraries = libraries,
        library = ":wasm",
        module = module,
        visibility = ["PUBLIC"],
    )

# A package's crate linked against libraries, every one in its closure, and
# exporting only what an app does: an app that is not an editor, such as
# crates/be-tool-tls-diamond-app.
def linked_app(module, libraries, rustc_flags = []):
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
        rustc_flags = guest_exports + rustc_flags + RELEASE_SYMBOL_FLAGS,
        srcs = cargo_library_sources(),
        target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"],
    )
    wasi_module(
        name = "module",
        libraries = libraries,
        library = ":wasm",
        module = module,
        visibility = ["PUBLIC"],
    )

# The package's app linked as :<name>, its module named module.wasm: what
# app() makes :module with, and a second build of the same app linked
# another way, such as triangle's :standalone. libraries = None links it
# against DEFAULT_LIBRARIES; a list names every library in its closure; [] links
# every crate it uses into it, a standalone app (guides/app_libraries.md, "The
# raw app interface"), a WASI reactor the host instantiates as it is.
def app_module(name, module, libraries = None, extra_deps = [], visibility = ["PUBLIC"]):
    facts = cargo_wasm_facts()
    if libraries == None:
        libraries = DEFAULT_LIBRARIES
    linker_flags = _needs(libraries)
    rustc_flags = [] if libraries else _reactor
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
        rustc_flags = guest_exports + rustc_flags + dev_only(["-Copt-level=0"]) + RELEASE_SYMBOL_FLAGS,
        srcs = cargo_library_sources() + ["manifest.json"],
        target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"],
        visibility = visibility,
    )
    wasi_module(
        name = name,
        libraries = libraries,
        library = ":" + library,
        module = module,
        visibility = visibility,
    )

# An app: the guest cdylib, :wasm (wasm32 only; an app is all behind
# cfg(target_arch = "wasm32")), :module named after its manifest's entry point,
# :manifest, and its wasm :test. test_env is extra environment for compiling the
# tests, which is how a test names a module it loads with include_bytes!.
# extra_deps are what cargo cannot name, such as a C library the app links.
# An app is linked against DEFAULT_LIBRARIES, or as libraries says
# (app_module above). types is the app's type module
# (a wasm32_module, such as checklist's //crates/be-type-checklist:module),
# which :types names for the app to stage beside :module.
#
# Outside a release build the app crate itself is compiled at opt-level 0,
# like its tests below, while its workspace dependencies are compiled at 1
# (buck/cargo/defs.bzl) and its third-party ones keep the app profile's 2.
# Most of an app's code is beui's generics instantiated anew, so a change
# under beui recompiles every app, and at opt-level 2 LLVM made that the
# longest part of ./scripts/verify: thirty apps took 1,130 seconds of
# compiling at 2 and 270 at 0. The modules are about two and a half times the
# size and take half as long again to precompile, and the app tests ran no
# slower.
#
# An app with examples/ is built for the host too (buck/cargo/buckify.sh
# plans it there), as the library named after its package, and each
# examples/NAME.rs is :example-NAME on the host, such as a benchmark that
# drives the editor through be-sdk-test.
def app(module, visibility = ["PUBLIC"], test_env = {}, data = {}, extra_deps = [], types = None, libraries = None):
    app_module(
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
    app_data(
        name = "data",
        files = data,
        visibility = visibility,
    )
    app_types(
        name = "types",
        module = types,
        visibility = visibility,
    )
    app_tests(
        env = test_env,
        exports = guest_exports,
        extra_deps = extra_deps,
    )
    examples = native.glob(["examples/*.rs"])
    if examples:
        cargo_library(extra_srcs = ["manifest.json"])
        for root in examples:
            cargo_example(root.removeprefix("examples/").removesuffix(".rs"))

# A crate's tests compiled to wasm and run by app-test-runner, which gives
# the module an app's imports; an editor's, be-sdk's and
# be-sdk-beui's.
#
# The test crate itself is compiled at opt-level 0 while its dependencies are
# compiled as the app's are. Every module instantiates beui's generics anew, so a
# change to beui recompiles about thirty of them at once, and at opt-level 2
# LLVM is most of that: the step took twice as long, though the tests run
# faster and the modules are smaller to precompile.
def app_tests(exports = [], env = {}, extra_deps = []):
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
        runner = "//crates/be-sdk-test-runner:bin",
    )
    app_test_run(
        name = "test-run",
        manifest = "Cargo.toml",
        module = ":test-module",
        paintings = native.glob(["snapshots/*.paint"]),
        runner = "//crates/be-sdk-test-runner:bin",
    )

# Compiled to wasm, an app's tests paint with the FreeType and HarfBuzz it
# ships, so the accepted paintings in the crate's snapshots/ stay put. The guest
# finds them from CARGO_MANIFEST_DIR, taken from the crate's real Cargo.toml
# rather than a staged copy, and app-test-runner opens that directory, and
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
                labels = ctx.attrs.labels + ["app"],
                run_from_project_root = True,
                type = "rust",
                use_project_relative_paths = False,
            ),
        )
        + [DefaultInfo(default_output = module)]
    )

# An app test's command and environment on a worker, where it draws through
# lavapipe, as the renderer's tests do, with LD_LIBRARY_PATH carried as
# BE3_LD_LIBRARY_PATH for the same reason (library_path_test in
# buck/cargo/defs.bzl).
def _on_worker(ctx: AnalysisContext, command: cmd_args, env: dict) -> (cmd_args, dict):
    sysroot = ctx.attrs._sysroot[DefaultInfo].default_outputs[0]
    return cmd_args("sh", "-c", 'LD_LIBRARY_PATH="$BE3_LD_LIBRARY_PATH" && export LD_LIBRARY_PATH && exec "$@"', "sh", command), env | {
        "BE3_LD_LIBRARY_PATH": cmd_args(sysroot, format = "{}/usr/lib/x86_64-linux-gnu"),
        "VK_ICD_FILENAMES": ctx.attrs._lavapipe,
    }

# What an app test runs, for buck2 test and for ./scripts/verify alike.
_app_test_attrs = {
    "manifest": attrs.source(),
    "module": attrs.transition_dep(cfg = wasi_transition),
    "paintings": attrs.list(attrs.source(), default = []),
    "precompile": attrs.bool(default = True),
    "_lavapipe": attrs.source(default = "root//buck/sysroot:lavapipe_icd.json"),
    "_sysroot": attrs.dep(default = "root//buck/sysroot:amd64-test"),
}

wasi_test = rule(
    attrs = _app_test_attrs
    | {
        "env": attrs.dict(default = {}, key = attrs.string(), value = attrs.arg()),
        "labels": attrs.list(attrs.string(), default = []),
        "runner": attrs.dep(providers = [RunInfo]),
        "_inject_test_env": attrs.default_only(attrs.dep(default = "prelude//test/tools:inject_test_env")),
    },
    impl = _wasi_test_impl,
)

# The app tests as an action, which is how ./scripts/verify runs them: on a
# worker, and answered from the cache when nothing they read has changed, which
# a test never is. They read the accepted paintings from the crate's
# snapshots/, and no other crate's, so accepting a painting leaves every other
# crate's tests in the cache. They accept every painting, writing the ones that
# changed or are new to changed/ and, for each accepted painting they compared,
# the SHA-256 of what they read to used/ (that of nothing when there was none),
# so that the paintings themselves are never downloaded; ./scripts/verify
# copies into the crate's snapshots/ the paintings in changed/ whose accepted
# painting still has that hash, or under --check fails on them.
def _app_test_run_impl(ctx: AnalysisContext) -> list[Provider]:
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
        category = "app_test",
        env = env,
    )
    return [DefaultInfo(default_output = paintings)]

app_test_run = rule(
    attrs = _app_test_attrs
    | {
        "runner": attrs.exec_dep(providers = [RunInfo]),
    },
    impl = _app_test_run_impl,
)
