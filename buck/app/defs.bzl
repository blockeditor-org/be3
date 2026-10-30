load("@root//buck/platforms:profile.bzl", "PROFILE_REFS", "keep_profile")
load("@root//buck/wasm:libraries.bzl", "WasmLibrariesInfo")

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

# A plugin's type module (guides/type_modules.md), or nothing for a plugin
# that brings no block type: what editor()'s :types is, so the app can stage
# every plugin's the same way.
def _plugin_types_impl(ctx: AnalysisContext) -> list[Provider]:
    if ctx.attrs.module:
        return [DefaultInfo(default_outputs = ctx.attrs.module[DefaultInfo].default_outputs)]
    return [DefaultInfo()]

plugin_types = rule(
    attrs = {
        "module": attrs.option(attrs.dep(), default = None),
    },
    impl = _plugin_types_impl,
)

# Every plugin's manifest and type module in one file (buck/app/bundle.sh), which
# an executable that is its whole release, `be`, carries with include_bytes!.
def _type_bundle_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output(ctx.label.name + ".bundle")
    command = cmd_args("sh", ctx.attrs._bundle, out.as_output())
    for manifest, types in zip(ctx.attrs.manifests, ctx.attrs.plugin_types):
        modules = types[DefaultInfo].default_outputs
        command.add(cmd_args(manifest, modules[0] if modules else "", delimiter = "="))
    ctx.actions.run(command, category = "type_bundle")
    return [DefaultInfo(default_output = out)]

type_bundle = rule(
    attrs = {
        "manifests": attrs.list(attrs.source()),
        "plugin_types": attrs.list(attrs.dep()),
        "_bundle": attrs.default_only(attrs.source(default = "root//buck/app:bundle.sh")),
    },
    impl = _type_bundle_impl,
)

# One action a module, so that the thirty-odd compiles run side by side on as
# many workers rather than one after another.
def _precompile(ctx: AnalysisContext, wasm: Artifact, name: [str, None] = None) -> Artifact:
    artifact = ctx.actions.declare_output((name or wasm.basename.removesuffix(".wasm")) + ".cwasm")
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
        identifier = name or wasm.basename,
    )
    return artifact

# The app as it runs (buck/app/stage.sh): the executable under cargo's name, and
# beside it every plugin, each module precompiled for precompile_target in an
# action of its own by the precompiler, plugin-test-runner as the workers run
# it, whatever the app is built for, and every library in their closures
# (guides/plugin_libraries.md), each with its .cwasm, and the type modules; with no
# precompiler, each plugin is its module alone. With no binary it
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
    libraries = []
    for module in ctx.attrs.modules + ctx.attrs.linked:
        info = module.get(WasmLibrariesInfo)
        for library in info.closure if info else []:
            if library not in libraries:
                libraries.append(library)
    for index, library in enumerate(libraries):
        command.add(cmd_args("--library=", library, delimiter = ""))
        if ctx.attrs.precompiler:
            artifact = "library{}-{}".format(index, library.basename.removesuffix(".wasm"))
            command.add(cmd_args("--library-artifact=", _precompile(ctx, library, artifact), delimiter = ""))
    for module in ctx.attrs.linked:
        command.add(cmd_args("--file=", module[DefaultInfo].default_outputs[0], delimiter = ""))
    data = ctx.attrs.data or [None] * len(ctx.attrs.manifests)
    plugin_types = ctx.attrs.plugin_types or [None] * len(ctx.attrs.manifests)
    for manifest, module, files, types in zip(ctx.attrs.manifests, ctx.attrs.modules, data, plugin_types):
        wasm = module[DefaultInfo].default_outputs[0]
        command.add(cmd_args(manifest, wasm, delimiter = "="))
        if files:
            command.add(cmd_args("--data=", files, delimiter = ""))
        if ctx.attrs.precompiler:
            command.add(cmd_args("--artifact=", _precompile(ctx, wasm), delimiter = ""))
        for type_module in types[DefaultInfo].default_outputs if types else []:
            command.add(cmd_args("--plugin-type=", type_module, delimiter = ""))
            if ctx.attrs.precompiler:
                command.add(cmd_args("--type-artifact=", _precompile(ctx, type_module), delimiter = ""))
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
        # Modules staged as they are, like files, with every library they
        # need: a linked module that is not a plugin with a manifest, such as
        # a test's.
        "linked": attrs.list(attrs.dep(), default = []),
        "manifests": attrs.list(attrs.source()),
        "modules": attrs.list(attrs.dep()),
        # Each plugin's type module (plugin_types), in the order of manifests:
        # staged beside its editor module under the name its manifest's
        # block_types give it.
        "plugin_types": attrs.list(attrs.dep(), default = []),
        "precompile_target": attrs.string(default = "x86_64-unknown-linux-gnu"),
        "precompiler": attrs.option(attrs.dep(providers = [RunInfo]), default = None),
        # What `buck2 run` runs a bundle without an executable with, followed
        # by the bundle.
        "run_with": attrs.list(attrs.arg(), default = []),
        "wasm_bindgen": attrs.option(attrs.exec_dep(providers = [RunInfo]), default = None),
        "_stage": attrs.default_only(attrs.source(default = "root//buck/app:stage.sh")),
    },
    impl = _app_impl,
)

# A page on beui's HTML core rendered at build time: the renderer, a program
# built for the host on that core (linux_x86_64_html), run with the path it
# writes the page to. A web bundle stages it as its index.html, which its wasm
# module then hydrates.
def _html_host_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.linux_x86_64_html[PlatformInfo], refs)

_html_host_transition = transition(
    impl = _html_host_transition_impl,
    refs = {"linux_x86_64_html": "root//buck/platforms:linux_x86_64_html"} | PROFILE_REFS,
)

def _prerendered_page_impl(ctx: AnalysisContext) -> list[Provider]:
    page = ctx.actions.declare_output(ctx.attrs.out)
    ctx.actions.run(cmd_args(ctx.attrs.renderer[RunInfo], page.as_output()), category = "prerender")
    return [DefaultInfo(default_output = page)]

prerendered_page = rule(
    attrs = {
        "out": attrs.string(default = "index.html"),
        "renderer": attrs.transition_dep(cfg = _html_host_transition),
    },
    impl = _prerendered_page_impl,
)

# A program on beui's HTML core for the host (linux_x86_64_html), from any
# configuration: be-server's executable (be-server-bin) renders its account portal on that
# core, so whatever stages, runs or deploys `//crates/be-server-bin:bin` gets it
# built there.
def _html_host_program_impl(ctx: AnalysisContext) -> list[Provider]:
    return [ctx.attrs.program[DefaultInfo], ctx.attrs.program[RunInfo]]

html_host_program = rule(
    attrs = {"program": attrs.transition_dep(cfg = _html_host_transition)},
    impl = _html_host_program_impl,
)
