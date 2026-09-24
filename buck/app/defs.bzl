# The app as it runs: its executable and every plugin beside it. buck/app/stage.py
# says what that layout is; this is the rule that makes it, and what
# `./scripts/buck run //crates/block-app:app` runs.
#
# A plugin is compiled for the host's own wasmtime ahead of time, by the plugin
# test runner's --precompile-to, which is block-wasm-host's own and so the
# engine the app loads it with, and which has to run where the action does. It
# is a target dependency rather than an exec one, as for the plugin tests, so
# that it is the same build of wasmtime theirs is; it is only asked for on the
# host, where the two configurations run on the same workers. A
# cross-compiled app gets its modules alone and compiles each at first launch,
# as the app does with any module it has no current .cwasm for.
def _app_impl(ctx: AnalysisContext) -> list[Provider]:
    executable = ctx.attrs.binary[DefaultInfo].default_outputs[0]
    out = ctx.actions.declare_output("app", dir = True)
    command = cmd_args("python3", ctx.attrs._stage, out.as_output(), executable)
    for manifest, module in zip(ctx.attrs.manifests, ctx.attrs.modules):
        wasm = module[DefaultInfo].default_outputs[0]
        command.add(cmd_args(manifest, wasm, delimiter = "="))
        if ctx.attrs.precompile:
            # One action a module, so that the thirty-odd compiles run side by
            # side on as many workers rather than one after another.
            artifact = ctx.actions.declare_output(wasm.basename.removesuffix(".wasm") + ".cwasm")
            ctx.actions.run(
                cmd_args(
                    ctx.attrs.precompiler[RunInfo],
                    "--precompile-to",
                    "--target",
                    ctx.attrs.precompile_target,
                    artifact.as_output(),
                    wasm,
                ),
                category = "wasm_precompile",
                identifier = wasm.basename,
            )
            command.add(cmd_args("--artifact=", artifact, delimiter = ""))
    ctx.actions.run(command, category = "app")
    return [
        DefaultInfo(default_output = out),
        RunInfo(args = cmd_args(out.project(executable.basename))),
    ]

app = rule(
    attrs = {
        "binary": attrs.dep(),
        "manifests": attrs.list(attrs.source()),
        "modules": attrs.list(attrs.dep()),
        "precompile": attrs.bool(default = True),
        "precompile_target": attrs.string(default = "x86_64-unknown-linux-gnu"),
        "precompiler": attrs.dep(providers = [RunInfo]),
        "_stage": attrs.default_only(attrs.source(default = "root//buck/app:stage.py")),
    },
    impl = _app_impl,
)
