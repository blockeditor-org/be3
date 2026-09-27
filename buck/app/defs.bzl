# A plugin's read-only data, which the app stages beside it under
# data/<plugin id>/ and a plugin reads through the host (ReadData): each file at
# files/<path>, and index.json listing every path.
_plugin_data = """
set -eu
out="$1"
shift
mkdir -p "$out/files"
paths=''
while [ "$#" -gt 0 ]; do
    mkdir -p "$(dirname "$out/files/$1")"
    cp "$2" "$out/files/$1"
    paths="$paths $1"
    shift 2
done
{
    printf '['
    separator=''
    for path in $paths; do
        printf '%s\\n  "%s"' "$separator" "$path"
        separator=','
    done
    [ -n "$paths" ] && printf '\\n'
    printf ']\\n'
} > "$out/index.json"
"""

def _plugin_data_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output(ctx.label.name, dir = True)
    command = cmd_args("sh", "-c", _plugin_data, "sh", out.as_output())
    for path in sorted(ctx.attrs.files):
        for segment in path.split("/"):
            if not segment or segment in [".", ".."] or not all([character.isalnum() or character in "._-" for character in segment.elems()]):
                fail("{} is not a plain relative path".format(path))
        command.add(path, ctx.attrs.files[path])
    ctx.actions.run(command, category = "plugin_data")
    return [DefaultInfo(default_output = out)]

plugin_data = rule(
    attrs = {
        "files": attrs.dict(attrs.string(), attrs.source(), default = {}),
    },
    impl = _plugin_data_impl,
)

# The app as it runs (buck/app/stage.sh): the executable under cargo's name, and
# beside it every plugin, each module precompiled for precompile_target in an
# action of its own by plugin-test-runner, block-wasm-host's own engine, which
# runs on the workers whatever the app is built for. With no binary it
# is the plugins alone. The web bundle is this rule too: `bindgen` runs
# wasm-bindgen over each module, and `index` writes the plugins.json a browser
# finds the plugins through.
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
    for extra_name, binary in ctx.attrs.extra_binaries.items():
        extra = binary[DefaultInfo].default_outputs[0]
        command.add(cmd_args("--executable=", extra, "=", extra_name + extra.extension, delimiter = ""))
    for file in ctx.attrs.files:
        command.add(cmd_args("--file=", file, delimiter = ""))
    data = ctx.attrs.data or [None] * len(ctx.attrs.manifests)
    for manifest, module, files in zip(ctx.attrs.manifests, ctx.attrs.modules, data):
        wasm = module[DefaultInfo].default_outputs[0]
        command.add(cmd_args(manifest, wasm, delimiter = "="))
        if files:
            command.add(cmd_args("--data=", files, delimiter = ""))
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
        # Each precompiled plugin without its module, for an app that only
        # falls back to the module.
        "compiled_only": attrs.bool(default = False),
        # Each plugin's read-only data (plugin_data), in the order of manifests.
        "data": attrs.list(attrs.source(), default = []),
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
