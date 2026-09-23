load("@prelude//test/inject_test_run_info.bzl", "inject_test_run_info")

# A WebAssembly module, named from a target that is not built for WebAssembly.
#
# The app never loads a game or a plugin as anything but a wasm module, and the
# tests that drive one are native: they run the host and hand it the module.
# Cargo arranges that with a build script that shells out to a second cargo
# build for wasm32 and prints the path it wrote to. Here the module is a target
# and the dependency is a dependency - the only unusual thing about it is that
# it is a dependency in a different configuration, which is what the transition
# below does.
#
# So a native rule can say:
#
#     env = {"GAME_WASM": "$(location //crates/.../tic_tac_toe:module)"}
#
# and get the wasm build of the same crate without knowing how it was made.
def _wasm32_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return refs.wasm32[PlatformInfo]

wasm32_transition = transition(
    impl = _wasm32_transition_impl,
    refs = {"wasm32": "root//buck/platforms:wasm32"},
)

# The guest's wasi, not the app's: the same triple with the constraint that
# tells reindeer which wgpu to resolve. buck/constraints/BUCK says why.
def _wasi_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return refs.wasi_guest[PlatformInfo]

wasi_transition = transition(
    impl = _wasi_transition_impl,
    refs = {"wasi_guest": "root//buck/platforms:wasi_guest"},
)

# rustc calls a cdylib a shared library whatever it is compiling for, so on
# wasm the "shared" output is the .wasm. Renaming it here is what makes the
# path a person reads in an error message the one they expected.
# A plugin's manifest names its entry point by file name and the app resolves
# it against the directory the manifest was found in, so the module attribute
# is how a target whose name is "module" still writes out checklist.wasm.
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

# rustc links a cdylib with --no-entry, which leaves the module without the
# symbols a host needs to stand one up: lld strips the ones that describe a
# thread's own storage, and there is no _start left to run libc's constructors
# through. The host lays out thread storage itself and calls __wasm_call_ctors
# itself, so a plugin has to export them. scripts/internal/common.sh passes the
# same list through RUSTFLAGS for the cargo build.
_plugin_exports = [
    "-Clink-arg=--export=__heap_base",
    "-Clink-arg=--export=__tls_base",
    "-Clink-arg=--export=__tls_size",
    "-Clink-arg=--export=__tls_align",
    "-Clink-arg=--export=__wasm_init_tls",
    "-Clink-arg=--export=__wasm_call_ctors",
]

# One editor: the guest, and the module the app loads it as.
#
# There is no native build of an editor - what a plugin is made of is behind
# cfg(target_arch = "wasm32") - so the library is compatible with wasm32 alone
# and asking for it on the host is a configuration error rather than a link
# failure. The module is named from the manifest's entry_point, which is what
# the app resolves against the directory it found the manifest in.
def editor(name, module, deps, test_deps = [], visibility = ["PUBLIC"]):
    native.rust_library(
        name = name + "_wasm",
        crate = name,
        crate_root = "src/lib.rs",
        deps = deps,
        edition = "2024",
        env = {
            "CARGO_CRATE_NAME": name,
            "CARGO_MANIFEST_DIR": "crates/editors/" + name,
            "CARGO_PKG_NAME": name,
            "CARGO_PKG_VERSION": "0.1.0",
        },
        preferred_linkage = "shared",
        rustc_flags = _plugin_exports,
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
    native.rust_binary(
        name = "test_module",
        crate = name,
        crate_root = "src/lib.rs",
        deps = deps + test_deps,
        edition = "2024",
        env = {
            "CARGO_CRATE_NAME": name,
            "CARGO_MANIFEST_DIR": "crates/editors/" + name,
            "CARGO_PKG_NAME": name,
            "CARGO_PKG_VERSION": "0.1.0",
        },
        rustc_flags = _plugin_exports + ["--test"],
        srcs = native.glob(["src/**/*.rs", "src/**/*.wgsl", "manifest.json"]),
        target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"],
    )
    wasi_test(
        name = "test",
        manifest = "Cargo.toml",
        module = ":test_module",
        runner = "//crates/plugin-test-runner:plugin-test-runner-bin",
    )

# An editor's tests, which are a wasm guest like the editor itself.
#
# A plugin paints with the FreeType and HarfBuzz it was compiled against, so
# running its tests natively paints with whatever those libraries happen to be
# on the machine and the accepted paintings never settle. Compiled to wasm they
# are the versions the plugin ships with. So the test binary is a module, built
# for wasi with --test the way cargo builds one, and the thing buck2 runs is the
# host: plugin-test-runner hands the module to wasmtime with a plugin's imports
# linked and opens a graphics device only if a test asks the gpu abi for one.
#
# CARGO_MANIFEST_DIR is what a plugin's tests find their accepted paintings
# through: the guest inherits the runner's environment, walks up from there to
# the workspace and reads snapshots/ under it. The runner preopens that same
# workspace for the guest, so the path has to be the real one rather than the
# staged copy a rustc action sees, which is why it is taken from the crate's
# Cargo.toml, which is a file in the repository rather than an output.
#
# Under cargo this is CARGO_TARGET_WASM32_WASIP1_THREADS_RUNNER; here the runner
# is an ordinary dependency, so it is built for whatever the test itself is
# built for, and the module is a dependency in wasi's configuration, which is
# what the transition below does. An exec_dep would put the runner in the
# execution platform's configuration instead and build wasmtime a second time
# for it.
# Cranelift is what makes a module this size take seconds rather than minutes,
# and it is still most of what a plugin test run costs: about forty seconds a
# module, and there are thirty-four of them. So the compile is an action here
# rather than something each test process does again - done once, shared by
# every test in the module, and answered by the cache on a machine that has
# never built the plugin.
#
# The compile is for the x86_64 baseline rather than for the machine it runs
# on. Left to itself wasmtime uses every CPU feature it finds, and the artifact
# is then only loadable by a machine with the same ones: the compile runs on a
# remote worker and the test on the machine that asked for it, and the two
# rarely match. Naming the target is what makes wasmtime stop looking.
#
# The runner is told where the artifact is rather than looking beside the
# module, because what decides whether one is stale under cargo is its mtime
# and buck2 does not preserve those. It does not need to: the runner is an
# input to the action that wrote the artifact, so a runner that could not read
# what it wrote is not a state that exists.
def _precompiled(ctx: AnalysisContext, module: Artifact) -> cmd_args:
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
            labels = ctx.attrs.labels,
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
        "runner": attrs.dep(providers = [RunInfo]),
        "_inject_test_env": attrs.default_only(attrs.dep(default = "prelude//test/tools:inject_test_env")),
    },
    impl = _wasi_test_impl,
)
