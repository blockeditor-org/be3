load("@root//buck/platforms:profile.bzl", "PROFILE_REFS", "keep_profile")
load("@root//buck/wasm:libraries.bzl", "WasmLibrariesInfo")

# An app's read-only data, which the app stages beside it under
# data/<app id>/ and an app reads through the host (ReadData): each file at
# files/<path>, and index.json listing every path.
def _app_data_impl(ctx: AnalysisContext) -> list[Provider]:
    paths = sorted(ctx.attrs.files)
    for path in paths:
        for segment in path.split("/"):
            if not segment or segment in [".", ".."] or not all([character.isalnum() or character in "._-" for character in segment.elems()]):
                fail("{} is not a plain relative path".format(path))
    contents = {"files/" + path: ctx.attrs.files[path] for path in paths}
    contents["index.json"] = ctx.actions.write_json("index.json", paths, pretty = True)
    return [DefaultInfo(default_output = ctx.actions.copied_dir(ctx.label.name, contents))]

app_data = rule(
    attrs = {
        "files": attrs.dict(attrs.string(), attrs.source(), default = {}),
    },
    impl = _app_data_impl,
)

# An app's type module (guides/type_modules.md), or nothing for an app
# that brings no block type: what app()'s :types is, so the app can stage
# every app's the same way.
def _app_types_impl(ctx: AnalysisContext) -> list[Provider]:
    if ctx.attrs.module:
        return [DefaultInfo(default_outputs = ctx.attrs.module[DefaultInfo].default_outputs)]
    return [DefaultInfo()]

app_types = rule(
    attrs = {
        "module": attrs.option(attrs.dep(), default = None),
    },
    impl = _app_types_impl,
)

# Every app's manifest and type module in one file (crates/be-tool-buck's bundle), which
# an executable that is its whole release, `be`, carries with include_bytes!.
def _type_bundle_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output(ctx.label.name + ".bundle")
    command = cmd_args(ctx.attrs._tools[RunInfo], "bundle", out.as_output())
    for manifest, types in zip(ctx.attrs.manifests, ctx.attrs.app_types):
        modules = types[DefaultInfo].default_outputs
        command.add(cmd_args(manifest, modules[0] if modules else "", delimiter = "="))
    ctx.actions.run(command, category = "type_bundle")
    return [DefaultInfo(default_output = out)]

type_bundle = rule(
    attrs = {
        "app_types": attrs.list(attrs.dep()),
        "manifests": attrs.list(attrs.source()),
        "_tools": attrs.default_only(attrs.exec_dep(default = "root//crates/be-tool-buck:bin", providers = [RunInfo])),
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

# Every app's modules, libraries and type modules precompiled once, for
# apps that stage some or all of them (crates/be-host-tests stages a few editors
# per test): an app given it as `precompiled` takes each .cwasm from here
# rather than precompiling its own, so apps that share a module share its
# action, which only runs once an app asks for its output.
PrecompiledInfo = provider(fields = {"artifacts": provider_field(dict)})

def _precompiled_apps_impl(ctx: AnalysisContext) -> list[Provider]:
    artifacts = {}
    for module in ctx.attrs.modules:
        info = module.get(WasmLibrariesInfo)
        for library in info.closure if info else []:
            if library not in artifacts:
                name = "library{}-{}".format(len(artifacts), library.basename.removesuffix(".wasm"))
                artifacts[library] = _precompile(ctx, library, name)
    for module in ctx.attrs.modules + ctx.attrs.app_types:
        for wasm in module[DefaultInfo].default_outputs[:1]:
            artifacts[wasm] = _precompile(ctx, wasm)
    return [DefaultInfo(default_outputs = artifacts.values()), PrecompiledInfo(artifacts = artifacts)]

precompiled_apps = rule(
    attrs = {
        "app_types": attrs.list(attrs.dep(), default = []),
        "modules": attrs.list(attrs.dep()),
        "precompile_target": attrs.string(default = "x86_64-unknown-linux-gnu"),
        "precompiler": attrs.dep(providers = [RunInfo]),
    },
    impl = _precompiled_apps_impl,
)

def _compiled(ctx: AnalysisContext, wasm: Artifact, name: [str, None] = None) -> Artifact:
    if not ctx.attrs.precompiled:
        return _precompile(ctx, wasm, name)
    artifacts = ctx.attrs.precompiled[PrecompiledInfo].artifacts
    if wasm not in artifacts:
        fail("{} is not among {}'s modules".format(wasm, ctx.attrs.precompiled.label))
    return artifacts[wasm]

# What `buck2 run` unpacks a program's archive with: built for the platform
# the program is, as the machine that runs it is, but on beui's native core,
# since a program on the HTML core, such as be-server's portal assets, is built
# for linux_x86_64_html and the unpacker is no crate of that core.
def _native_core_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    html = refs.html[ConstraintValueInfo]
    if platform.configuration.constraints.get(html.setting.label) != html:
        return platform
    return keep_profile(platform, refs.linux_x86_64[PlatformInfo], refs)

_native_core_transition = transition(
    impl = _native_core_transition_impl,
    refs = {"html": "root//buck/constraints:html", "linux_x86_64": "root//buck/platforms:linux_x86_64"} | PROFILE_REFS,
)

# A program's directory from the stage's layout: each file the stage wrote, the
# executable, which finds the apps beside the path it was run from, and each
# source file, which can change under a link to it (buck/program/build-info.json
# does with every command), copied; every other file a symlink to the artifact
# an argument gave, or to the file within it for a --tree or --data directory.
def _lay_out(ctx: AnalysisContext, layout: list, arguments: list, generated: Artifact, executable: [str, None], out: Artifact):
    contents = {}
    for entry in layout:
        path = entry["path"]
        if "argument" not in entry:
            contents[path] = assembled_dir.copy(generated.project(path))
            continue
        artifact = arguments[entry["argument"]][entry["part"]]
        if entry["within"]:
            artifact = artifact.project(entry["within"])
        copied = path == executable or artifact.is_source
        contents[path] = assembled_dir.copy(artifact) if copied else assembled_dir.symlink(artifact)
    ctx.actions.assembled_dir(out, contents = contents, has_content_based_path = False)

# The app as it runs (crates/be-tool-buck's stage): the executable under cargo's name, and
# beside it every app, each module precompiled for precompile_target in an
# action of its own by the precompiler, app-test-runner as the workers run
# it, whatever the app is built for, and every library in their closures
# (guides/app_libraries.md), each with its .cwasm, and the type modules; with no
# precompiler, each app is its module alone. With no binary it
# is the apps alone. `index` writes the apps.json the app finds the
# apps through. The web bundle is this rule too: `bindgen` runs
# wasm-bindgen over each module. The stage writes only the files it makes
# and where everything goes; the directory is then laid out from that
# (_lay_out), each file a symlink to the artifact it is, so programs that
# share the apps' modules and .cwasm files share their bytes too. `[archive]`
# is the whole directory, links followed, as one
# compressed file (crates/be-host-archive), a quarter of its size or less, made on a
# worker. `buck2 run` runs the executable from it, unpacked into target/programs
# once for each new archive, so a run downloads only the archive; or, for a
# bundle without one, `run_with` followed by the bundle, which is how the web
# bundle serves itself.
def _program_impl(ctx: AnalysisContext) -> list[Provider]:
    generated = ctx.actions.declare_output(ctx.label.name + "-generated", dir = True)
    layout = ctx.actions.declare_output(ctx.label.name + "-layout.json")
    command = cmd_args(ctx.attrs._tools[RunInfo], "stage", generated.as_output(), layout.as_output())

    # Each argument's artifacts, in the order the stage numbers them. The stage reads
    # only what it is given by path; the rest it needs only the names of.
    arguments = []

    def add(argument, artifacts = []):
        command.add(argument)
        arguments.append(artifacts)

    only_libraries = ctx.attrs.libraries == "only"
    if ctx.attrs.compiled_only:
        add("--compiled-only")
    name = None
    if ctx.attrs.binary:
        executable = ctx.attrs.binary[DefaultInfo].default_outputs[0]
        name = ctx.attrs.executable_name + executable.extension
        add("--executable={}={}".format(executable.basename, name), [executable])
    for file in ctx.attrs.files:
        add("--file=" + file.basename, [file])
    libraries = []
    for module in ctx.attrs.modules + ctx.attrs.linked:
        info = module.get(WasmLibrariesInfo)
        for library in info.closure if info else []:
            if library not in libraries:
                libraries.append(library)
    for index, library in enumerate(libraries if ctx.attrs.libraries != "exclude" else []):
        add(cmd_args("--library=", library, delimiter = ""), [library])
        if ctx.attrs.precompiler or ctx.attrs.precompiled:
            artifact = _compiled(ctx, library, "library{}-{}".format(index, library.basename.removesuffix(".wasm")))
            add("--library-artifact=" + artifact.basename, [artifact])
    for module in ctx.attrs.linked:
        wasm = module[DefaultInfo].default_outputs[0]
        add("--file=" + wasm.basename, [wasm])
    data = ctx.attrs.data or [None] * len(ctx.attrs.manifests)
    app_types = ctx.attrs.app_types or [None] * len(ctx.attrs.manifests)
    for manifest, module, files, types in zip(ctx.attrs.manifests, ctx.attrs.modules, data, app_types):
        if only_libraries:
            break
        wasm = module[DefaultInfo].default_outputs[0]
        add(cmd_args(manifest, wasm.basename, delimiter = "="), [manifest, wasm])
        if files:
            add(cmd_args("--data=", files, delimiter = ""), [files])
        if ctx.attrs.precompiler or ctx.attrs.precompiled:
            artifact = _compiled(ctx, wasm)
            add("--artifact=" + artifact.basename, [artifact])
        for type_module in types[DefaultInfo].default_outputs if types else []:
            add(cmd_args("--app-type=", type_module, delimiter = ""), [type_module])
            if ctx.attrs.precompiler or ctx.attrs.precompiled:
                artifact = _compiled(ctx, type_module)
                add("--type-artifact=" + artifact.basename, [artifact])
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
        add(cmd_args("--tree=", bindings, delimiter = ""), [bindings])
    if ctx.attrs.index:
        add("--index")
    ctx.actions.run(command, category = "program")

    # Not content-based: buck2 would lay the directory out once and copy it to a path
    # named for its hash, so the executable, the one file copied, would take twice the space.
    out = ctx.actions.declare_output(ctx.label.name, dir = True, has_content_based_path = False)
    ctx.actions.dynamic_output(
        dynamic = [layout],
        inputs = [],
        outputs = [out.as_output()],
        f = lambda ctx, artifacts, outputs: _lay_out(ctx, artifacts[layout].read_json(), arguments, generated, name, outputs[out]),
    )
    archive = ctx.actions.declare_output(ctx.label.name + ".programz")
    ctx.actions.run(
        cmd_args(ctx.attrs._archiver[RunInfo], "pack", out, archive.as_output()),
        category = "program_archive",
    )
    providers = [
        DefaultInfo(
            default_output = out,
            sub_targets = {"archive": [DefaultInfo(default_output = archive)]},
        )
    ]
    if name:
        providers.append(RunInfo(args = cmd_args(ctx.attrs._unpacker[RunInfo], "run", archive, name)))
    elif ctx.attrs.run_with:
        providers.append(RunInfo(args = cmd_args(ctx.attrs.run_with, out)))
    return providers

program = rule(
    attrs = {
        # Each app's type module (app_types), in the order of manifests:
        # staged beside its editor module under the name its manifest's
        # block_types give it.
        "app_types": attrs.list(attrs.dep(), default = []),
        "binary": attrs.option(attrs.dep(), default = None),
        "bindgen": attrs.list(attrs.dep(), default = []),
        # Each precompiled app without its module, for an app that only
        # falls back to the module.
        "compiled_only": attrs.bool(default = False),
        # Each app's read-only data (app_data), in the order of manifests.
        "data": attrs.list(attrs.source(), default = []),
        "executable_name": attrs.string(default = ""),
        "files": attrs.list(attrs.source(), default = []),
        "index": attrs.bool(default = False),
        # Which libraries the program holds: "include" every one the modules
        # need, "exclude" them, for a program run with the BE_BUNDLE path list
        # (be-lib-build-id) that has them in another, or "only" them and none of
        # the apps, which is that other directory.
        "libraries": attrs.enum(["include", "exclude", "only"], default = "include"),
        # Modules staged as they are, like files, with every library they
        # need: a linked module that is not an app with a manifest, such as
        # a test's.
        "linked": attrs.list(attrs.dep(), default = []),
        "manifests": attrs.list(attrs.source()),
        "modules": attrs.list(attrs.dep()),
        "precompile_target": attrs.string(default = "x86_64-unknown-linux-gnu"),
        # Where each .cwasm comes from in place of the precompiler: a
        # precompiled_apps target over these modules or more.
        "precompiled": attrs.option(attrs.dep(providers = [PrecompiledInfo]), default = None),
        "precompiler": attrs.option(attrs.dep(providers = [RunInfo]), default = None),
        # What `buck2 run` runs a bundle without an executable with, followed
        # by the bundle.
        "run_with": attrs.list(attrs.arg(), default = []),
        "wasm_bindgen": attrs.option(attrs.exec_dep(providers = [RunInfo]), default = None),
        "_archiver": attrs.default_only(attrs.exec_dep(default = "root//crates/be-host-archive:bin")),
        "_tools": attrs.default_only(attrs.exec_dep(default = "root//crates/be-tool-buck:bin", providers = [RunInfo])),
        "_unpacker": attrs.default_only(attrs.transition_dep(cfg = _native_core_transition, default = "root//crates/be-host-archive:bin")),
    },
    impl = _program_impl,
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
