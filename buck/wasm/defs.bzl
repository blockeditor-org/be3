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
def _wasm_module_impl(ctx: AnalysisContext) -> list[Provider]:
    shared = ctx.attrs.library[DefaultInfo].sub_targets["shared"][DefaultInfo].default_outputs[0]
    module = ctx.actions.copy_file(ctx.attrs.name + ".wasm", shared)
    return [DefaultInfo(default_output = module)]

wasm32_module = rule(
    attrs = {"library": attrs.transition_dep(cfg = wasm32_transition)},
    impl = _wasm_module_impl,
)

wasi_module = rule(
    attrs = {"library": attrs.transition_dep(cfg = wasi_transition)},
    impl = _wasm_module_impl,
)
