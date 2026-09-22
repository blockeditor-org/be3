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

def _wasi_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return refs.wasi[PlatformInfo]

wasi_transition = transition(
    impl = _wasi_transition_impl,
    refs = {"wasi": "root//buck/platforms:wasi"},
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
def editor(name, module, deps, visibility = ["PUBLIC"]):
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
