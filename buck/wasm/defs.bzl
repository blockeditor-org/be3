load("@prelude//test/inject_test_run_info.bzl", "inject_test_run_info")
load("@root//buck/cargo:defs.bzl", "cargo_wasm_facts")
load("@root//buck/platforms:cross.bzl", "per_cross_platform")
load("@root//buck/platforms:profile.bzl", "PROFILE_REFS", "keep_profile")

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

# A cdylib's "shared" output is the .wasm; this names it as the module it is,
# which for a plugin is the entry point its manifest names.
def _wasm_module_impl(ctx: AnalysisContext) -> list[Provider]:
    shared = ctx.attrs.library[DefaultInfo].sub_targets["shared"][DefaultInfo].default_outputs[0]
    module = ctx.actions.copy_file((ctx.attrs.module or ctx.attrs.name) + ".wasm", shared)
    return [DefaultInfo(default_output = module)]

wasm32_module = rule(
    attrs = {
        "library": attrs.transition_dep(cfg = wasm32_transition),
        "module": attrs.option(attrs.string(), default = None),
    },
    impl = _wasm_module_impl,
)

wasi_module = rule(
    attrs = {
        "library": attrs.transition_dep(cfg = wasi_transition),
        "module": attrs.option(attrs.string(), default = None),
    },
    impl = _wasm_module_impl,
)

wasi_app_module = rule(
    attrs = {
        "library": attrs.transition_dep(cfg = wasi_app_transition),
        "module": attrs.option(attrs.string(), default = None),
    },
    impl = _wasm_module_impl,
)

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

# An editor: the guest cdylib (wasm32 only; a plugin is all behind
# cfg(target_arch = "wasm32")), :module named after its manifest's entry point,
# :manifest, and its wasm :test.
def editor(name, module, visibility = ["PUBLIC"]):
    facts = cargo_wasm_facts()
    native.rust_library(
        name = name + "_wasm",
        crate = facts.crate,
        crate_root = facts.crate_root,
        deps = facts.deps,
        edition = facts.edition,
        env = facts.env,
        features = facts.features,
        preferred_linkage = "shared",
        rustc_flags = plugin_exports,
        srcs = native.glob(["src/**/*.rs", "src/**/*.wgsl", "manifest.json"]),
        target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"],
        visibility = visibility,
    )
    wasi_module(
        name = "module",
        library = ":" + name + "_wasm",
        module = module,
        visibility = visibility,
    )
    native.export_file(
        name = "manifest",
        src = "manifest.json",
        visibility = visibility,
    )
    plugin_tests(
        exports = plugin_exports,
        srcs = native.glob(["src/**/*.rs", "src/**/*.wgsl", "manifest.json"]),
    )

# A crate's tests compiled to wasm and run by plugin-test-runner, which gives
# the module a plugin's imports; an editor's, and block-editor-plugin's.
def plugin_tests(srcs, exports = []):
    facts = cargo_wasm_facts()
    native.rust_binary(
        name = "test_module",
        crate = facts.crate,
        crate_root = facts.crate_root,
        deps = facts.test_deps,
        edition = facts.edition,
        env = facts.env,
        features = facts.test_features,
        rustc_flags = exports + ["--test"],
        srcs = srcs,
        target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"],
    )
    wasi_test(
        name = "test",
        manifest = "Cargo.toml",
        module = ":test_module",
        precompile = per_cross_platform(True, lambda _: False),
        runner = "//crates/plugin-test-runner:plugin-test-runner-bin",
    )

# Compiled to wasm, a plugin's tests paint with the FreeType and HarfBuzz it
# ships, so the accepted paintings in snapshots/ stay put. The guest finds them
# from CARGO_MANIFEST_DIR, taken from the crate's real Cargo.toml rather than a
# staged copy.
#
# The module is compiled for wasmtime once, in an action, rather than by every
# test process - about forty seconds a module. It is compiled for the x86_64
# baseline, since the compile runs on a worker and the test here, and only for
# the host: elsewhere the runner compiles it when the test runs.
def _precompiled(ctx: AnalysisContext, module: Artifact) -> cmd_args:
    if not ctx.attrs.precompile:
        return cmd_args(module)
    runner = ctx.attrs.runner[RunInfo]
    artifact = ctx.actions.declare_output(module.basename.removesuffix(".wasm") + ".cwasm")
    ctx.actions.run(
        cmd_args(runner, "--precompile-to", "--target", "x86_64-unknown-linux-gnu", artifact.as_output(), module),
        category = "wasm_precompile",
        identifier = module.basename,
    )
    return cmd_args("--precompiled", artifact, module)

def _wasi_test_impl(ctx: AnalysisContext) -> list[Provider]:
    module = ctx.attrs.module[DefaultInfo].default_outputs[0]
    command = cmd_args(ctx.attrs.runner[RunInfo], _precompiled(ctx, module))
    env = dict(ctx.attrs.env)
    if ctx.attrs.manifest:
        env["CARGO_MANIFEST_DIR"] = cmd_args(ctx.attrs.manifest, parent = 1)
    return inject_test_run_info(
        ctx,
        ExternalRunnerTestInfo(
            command = [command],
            env = env,
            # What ./scripts/buck run //:verify tells the plugin tests apart by: they run
            # here and accept paintings, and the rest run on a worker.
            labels = ctx.attrs.labels + ["plugin"],
            run_from_project_root = True,
            type = "rust",
            use_project_relative_paths = False,
        ),
    ) + [DefaultInfo(default_output = module)]

wasi_test = rule(
    attrs = {
        "env": attrs.dict(default = {}, key = attrs.string(), value = attrs.arg()),
        "labels": attrs.list(attrs.string(), default = []),
        "manifest": attrs.option(attrs.source(), default = None),
        "module": attrs.transition_dep(cfg = wasi_transition),
        "precompile": attrs.bool(default = True),
        "runner": attrs.dep(providers = [RunInfo]),
        "_inject_test_env": attrs.default_only(attrs.dep(default = "prelude//test/tools:inject_test_env")),
    },
    impl = _wasi_test_impl,
)
