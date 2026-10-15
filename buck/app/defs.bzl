# A plugin's read-only data, which the app stages beside it under
# data/<plugin id>/ and a plugin reads through the host (ReadData): each file at
# files/<path>, and index.json listing every path.
def _plugin_data_impl(ctx: AnalysisContext) -> list[Provider]:
    paths = sorted(ctx.attrs.files)
    for path in paths:
        for segment in path.split("/"):
            if not segment or segment in [".", ".."] or not all([character.isalnum() or character in "._-" for character in segment.elems()]):
                fail("{} is not a plain relative path".format(path))
    contents = {"files/" + path: ctx.attrs.files[path] for path in paths}
    contents["index.json"] = ctx.actions.write_json("index.json", paths, pretty = True)
    return [DefaultInfo(default_output = ctx.actions.copied_dir(ctx.label.name, contents))]

plugin_data = rule(
    attrs = {
        "files": attrs.dict(attrs.string(), attrs.source(), default = {}),
    },
    impl = _plugin_data_impl,
)

# One action a module, so that the thirty-odd compiles run side by side on as
# many workers rather than one after another.
def _precompile(ctx: AnalysisContext, wasm: Artifact) -> Artifact:
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
    return artifact

# The app as it runs (buck/app/stage.sh): the executable under cargo's name, and
# beside it every plugin, each module precompiled for precompile_target in an
# action of its own by the precompiler, plugin-test-runner as the workers run
# it, whatever the app is built for, and the plugin SDK they are linked against
# (guides/plugin_sdk.md), with its .cwasm; with no precompiler, each plugin is
# its module alone. With no binary it
# is the plugins alone. `index` writes the plugins.json the app finds the
# plugins through. The web bundle is this rule too: `bindgen` runs
# wasm-bindgen over each module. `buck2 run` runs the executable, or for a
# bundle without one, `run_with` followed by the bundle, which is how the web
# bundle serves itself.
def _app_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output(ctx.label.name, dir = True)
    command = cmd_args("sh", ctx.attrs._stage, out.as_output())
    if ctx.attrs.compiled_only:
        command.add("--compiled-only")
    name = None
    if ctx.attrs.binary:
        executable = ctx.attrs.binary[DefaultInfo].default_outputs[0]
        name = ctx.attrs.executable_name + executable.extension
        command.add(cmd_args("--executable=", executable, "=", name, delimiter = ""))
    for file in ctx.attrs.files:
        command.add(cmd_args("--file=", file, delimiter = ""))
    if ctx.attrs.sdk:
        sdk = ctx.attrs.sdk[DefaultInfo].default_outputs[0]
        command.add(cmd_args("--sdk=", sdk, delimiter = ""))
        if ctx.attrs.precompiler:
            command.add(cmd_args("--sdk-artifact=", _precompile(ctx, sdk), delimiter = ""))
    data = ctx.attrs.data or [None] * len(ctx.attrs.manifests)
    for manifest, module, files in zip(ctx.attrs.manifests, ctx.attrs.modules, data):
        wasm = module[DefaultInfo].default_outputs[0]
        command.add(cmd_args(manifest, wasm, delimiter = "="))
        if files:
            command.add(cmd_args("--data=", files, delimiter = ""))
        if ctx.attrs.precompiler:
            command.add(cmd_args("--artifact=", _precompile(ctx, wasm), delimiter = ""))
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
    elif ctx.attrs.run_with:
        providers.append(RunInfo(args = cmd_args(ctx.attrs.run_with, out)))
    return providers

app = rule(
    attrs = {
        "binary": attrs.option(attrs.dep(), default = None),
        "bindgen": attrs.list(attrs.dep(), default = []),
        # Each precompiled plugin without its module, for an app that only
        # falls back to the module.
        "compiled_only": attrs.bool(default = False),
        # Each plugin's read-only data (plugin_data), in the order of manifests.
        "data": attrs.list(attrs.source(), default = []),
        "executable_name": attrs.string(default = ""),
        "files": attrs.list(attrs.source(), default = []),
        "index": attrs.bool(default = False),
        "manifests": attrs.list(attrs.source()),
        "modules": attrs.list(attrs.dep()),
        "precompile_target": attrs.string(default = "x86_64-unknown-linux-gnu"),
        "precompiler": attrs.option(attrs.dep(providers = [RunInfo]), default = None),
        # What `buck2 run` runs a bundle without an executable with, followed
        # by the bundle.
        "run_with": attrs.list(attrs.arg(), default = []),
        # The plugin SDK the modules are linked against, staged under the name
        # they find it by.
        "sdk": attrs.option(attrs.dep(), default = None),
        "wasm_bindgen": attrs.option(attrs.exec_dep(providers = [RunInfo]), default = None),
        "_stage": attrs.default_only(attrs.source(default = "root//buck/app:stage.sh")),
    },
    impl = _app_impl,
)
