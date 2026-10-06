load("@prelude//test/inject_test_run_info.bzl", "inject_test_run_info")
load("@root//buck/app:defs.bzl", "plugin_data")
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
# :manifest, and its wasm :test. test_env is extra environment for compiling the
# tests, which is how a test names a module it loads with include_bytes!.
# extra_deps are what cargo cannot name, such as a C library the plugin links.
def editor(name, module, visibility = ["PUBLIC"], test_env = {}, data = {}, extra_deps = []):
    facts = cargo_wasm_facts()
    native.rust_library(
        name = name + "_wasm",
        crate = facts.crate,
        crate_root = facts.crate_root,
        deps = facts.deps + extra_deps,
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
    plugin_data(
        name = "data",
        files = data,
        visibility = visibility,
    )
    plugin_tests(
        env = test_env,
        exports = plugin_exports,
        extra_deps = extra_deps,
        srcs = native.glob(["src/**/*.rs", "src/**/*.wgsl", "manifest.json"]),
    )

# A crate's tests compiled to wasm and run by plugin-test-runner, which gives
# the module a plugin's imports; an editor's, block-editor-plugin's and
# block-editor-beui's.
#
# The test crate itself is compiled at opt-level 0 while its dependencies keep
# the plugin profile's 2. Every module instantiates beui's generics anew, so a
# change to beui recompiles about thirty of them at once, and at opt-level 2
# LLVM is most of that: the step took twice as long, though the tests run
# faster and the modules are smaller to precompile.
def plugin_tests(srcs, exports = [], env = {}, extra_deps = []):
    facts = cargo_wasm_facts()
    native.rust_binary(
        name = "test_module",
        crate = facts.crate,
        crate_root = facts.crate_root,
        deps = facts.test_deps + extra_deps,
        edition = facts.edition,
        env = facts.env | env,
        features = facts.test_features,
        rustc_flags = exports + ["--test", "-Copt-level=0"],
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
    plugin_test_run(
        name = "test_run",
        manifest = "Cargo.toml",
        module = ":test_module",
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

# What a plugin test runs, for buck2 test and for //:verify alike.
_plugin_test_attrs = {
    "manifest": attrs.source(),
    "module": attrs.transition_dep(cfg = wasi_transition),
    "precompile": attrs.bool(default = True),
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

# The plugin tests as an action, which is how //:verify runs them: on a worker,
# and answered from the cache when nothing they read has changed, which a test
# never is. They read the accepted paintings from snapshots/ and accept every
# painting, writing the ones that changed or are new to changed/ and naming the
# ones they compared in used/; //:verify copies changed/ into snapshots/, or
# under --check fails on it. They draw through lavapipe, as the renderer's tests
# do, with LD_LIBRARY_PATH carried as BE3_LD_LIBRARY_PATH for the same reason
# (library_path_test in buck/cargo/defs.bzl).
def _plugin_test_run_impl(ctx: AnalysisContext) -> list[Provider]:
    module = ctx.attrs.module[DefaultInfo].default_outputs[0]
    paintings = ctx.actions.declare_output("paintings", dir = True, has_content_based_path = False)
    sysroot = ctx.attrs._sysroot[DefaultInfo].default_outputs[0]
    ctx.actions.run(
        cmd_args(
            "sh",
            "-c",
            'mkdir -p "$1/used" "$1/changed" && USED_PAINTINGS="$1/used" CHANGED_PAINTINGS="$1/changed" LD_LIBRARY_PATH="$BE3_LD_LIBRARY_PATH" && export USED_PAINTINGS CHANGED_PAINTINGS LD_LIBRARY_PATH && shift && exec "$@"',
            "sh",
            paintings.as_output(),
            ctx.attrs.runner[RunInfo],
            _precompiled(ctx, module),
            hidden = ctx.attrs._inputs[DefaultInfo].default_outputs,
        ),
        category = "plugin_test",
        env = {
            "BE3_LD_LIBRARY_PATH": cmd_args(sysroot, format = "{}/usr/lib/x86_64-linux-gnu"),
            "CARGO_MANIFEST_DIR": cmd_args(ctx.attrs.manifest, parent = 1),
            "UPDATE_SNAPSHOTS": "1",
            "VK_ICD_FILENAMES": ctx.attrs._lavapipe,
        },
    )
    return [DefaultInfo(default_output = paintings)]

plugin_test_run = rule(
    attrs = _plugin_test_attrs
    | {
        "runner": attrs.exec_dep(providers = [RunInfo]),
        "_inputs": attrs.dep(default = "root//:plugin_test_inputs"),
        "_lavapipe": attrs.source(default = "root//buck/sysroot:lavapipe_icd.json"),
        "_sysroot": attrs.dep(default = "root//buck/sysroot:amd64-test"),
    },
    impl = _plugin_test_run_impl,
)

plugin_test_inputs = rule(
    attrs = {"srcs": attrs.list(attrs.source())},
    impl = lambda ctx: [DefaultInfo(default_outputs = ctx.attrs.srcs)],
)
