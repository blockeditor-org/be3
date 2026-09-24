# The app as it runs (buck/app/stage.sh): the executable under cargo's name, and
# beside it every plugin, each module precompiled in an action of its own by
# plugin-test-runner, block-wasm-host's own engine, on the host only;
# cross-compiled apps compile their modules at first launch. With no binary it
# is the plugins alone. The web bundle is this rule too: `bindgen` runs
# wasm-bindgen over each module, and `index` writes the plugins.json a browser
# finds the plugins through.
def _app_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output(ctx.label.name, dir = True)
    command = cmd_args("sh", ctx.attrs._stage, out.as_output())
    name = None
    if ctx.attrs.binary:
        executable = ctx.attrs.binary[DefaultInfo].default_outputs[0]
        name = ctx.attrs.executable_name + executable.extension
        command.add(cmd_args("--executable=", executable, "=", name, delimiter = ""))
    for extra_name, binary in ctx.attrs.extra_binaries.items():
        extra = binary[DefaultInfo].default_outputs[0]
        command.add(cmd_args("--executable=", extra, "=", extra_name + extra.extension, delimiter = ""))
    for file in ctx.attrs.files:
        command.add(cmd_args("--file=", file, delimiter = ""))
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
    for module in ctx.attrs.bindgen:
        wasm = module[DefaultInfo].default_outputs[0]
        bindings = ctx.actions.declare_output(wasm.basename.removesuffix(".wasm") + "-bindings", dir = True)
        ctx.actions.run(
            cmd_args(
                ctx.attrs.wasm_bindgen[RunInfo],
                "--target",
                "web",
                "--no-typescript",
                "--out-dir",
                bindings.as_output(),
                wasm,
            ),
            category = "wasm_bindgen",
            identifier = wasm.basename,
        )
        command.add(cmd_args("--tree=", bindings, delimiter = ""))
    if ctx.attrs.index:
        command.add("--index")
    ctx.actions.run(command, category = "app")
    providers = [DefaultInfo(default_output = out)]
    if name:
        providers.append(RunInfo(args = cmd_args(out.project(name))))
    return providers

app = rule(
    attrs = {
        "binary": attrs.option(attrs.dep(), default = None),
        "bindgen": attrs.list(attrs.dep(), default = []),
        "executable_name": attrs.string(default = ""),
        # More executables to put beside the app, by the name cargo gives each.
        "extra_binaries": attrs.dict(attrs.string(), attrs.dep(), default = {}),
        "files": attrs.list(attrs.source(), default = []),
        "index": attrs.bool(default = False),
        "manifests": attrs.list(attrs.source()),
        "modules": attrs.list(attrs.dep()),
        "precompile": attrs.bool(default = True),
        "precompile_target": attrs.string(default = "x86_64-unknown-linux-gnu"),
        "precompiler": attrs.dep(providers = [RunInfo]),
        "wasm_bindgen": attrs.option(attrs.exec_dep(providers = [RunInfo]), default = None),
        "_stage": attrs.default_only(attrs.source(default = "root//buck/app:stage.sh")),
    },
    impl = _app_impl,
)
