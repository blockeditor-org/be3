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

# The SHA-256 of every file of a program's directory, as app-hashes.json
# (crates/be-tool-buck's hashes): what a release publishes beside its apps, so
# `be` can check each app file it downloads from it.
def _bundle_hashes_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output("app-hashes.json")
    ctx.actions.run(
        cmd_args(ctx.attrs._tools[RunInfo], "hashes", out.as_output(), ctx.attrs.dir[DefaultInfo].default_outputs[0]),
        category = "bundle_hashes",
    )
    return [DefaultInfo(default_output = out)]

bundle_hashes = rule(
    attrs = {
        "dir": attrs.dep(),
        "_tools": attrs.default_only(attrs.exec_dep(default = "root//crates/be-tool-buck:bin", providers = [RunInfo])),
    },
    impl = _bundle_hashes_impl,
)

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
# `packed` is what the archive packs: the same, every file a symlink, without the
# arguments numbered in `beside`.
def _lay_out(ctx: AnalysisContext, layout: list, arguments: list, beside: list, generated: Artifact, executable: [str, None], out: Artifact, packed: Artifact):
    contents = {}
    packed_contents = {}
    for entry in layout:
        path = entry["path"]
        if "argument" not in entry:
            contents[path] = assembled_dir.copy(generated.project(path))
            packed_contents[path] = assembled_dir.symlink(generated.project(path))
            continue
        artifact = arguments[entry["argument"]][entry["part"]]
        if entry["within"]:
            artifact = artifact.project(entry["within"])
        copied = path == executable or artifact.is_source
        contents[path] = assembled_dir.copy(artifact) if copied else assembled_dir.symlink(artifact)
        if entry["argument"] not in beside:
            packed_contents[path] = assembled_dir.symlink(artifact)
    ctx.actions.assembled_dir(out, contents = contents, has_content_based_path = False)
    ctx.actions.assembled_dir(packed, contents = packed_contents)

# The app as it runs (crates/be-tool-buck's stage): the executable under cargo's name, and
# beside it every app's module, every library in their closures
# (guides/app_libraries.md) and the type modules. With no binary it
# is the apps alone. `index` writes the apps.json the app finds the
# apps through. The web bundle is this rule too: `bindgen` runs
# wasm-bindgen over each module. The stage writes only the files it makes
# and where everything goes; the directory is then laid out from that
# (_lay_out), each file a symlink to the artifact it is, so programs that
# share the apps' modules share their bytes too. `[archive]`
# is the whole directory (but an executable's source files), links followed, as one
# compressed file (crates/be-host-archive), a quarter of its size or less, made on a
# worker. `buck2 run` runs the executable from it, unpacked into target/apps
# once for each new archive, with the source files copied in (--beside), so a run
# downloads only the archive and those; or, for a
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
    name = None
    if ctx.attrs.binary:
        executable = ctx.attrs.binary[DefaultInfo].default_outputs[0]
        name = ctx.attrs.executable_name + executable.extension
        add("--executable={}={}".format(executable.basename, name), [executable])
    # In a program with an executable, a source file among `files` is kept out of
    # the archive and copied in beside it when `buck2 run` unpacks it:
    # buck/program/build-info.json changes with every build id, which is every
    # change to the working tree, and in the archive it would pack, download and
    # unpack the whole program again for it. Any other program's archive is whole.
    beside = []
    for file in ctx.attrs.files:
        if ctx.attrs.binary and file.is_source:
            beside.append(len(arguments))
        add("--file=" + file.basename, [file])
    libraries = []
    for module in [module for module in ctx.attrs.modules if module] + ctx.attrs.linked:
        info = module.get(WasmLibrariesInfo)
        for library in info.closure if info else []:
            if library not in libraries:
                libraries.append(library)
    for library in libraries if ctx.attrs.libraries != "exclude" else []:
        add(cmd_args("--library=", library, delimiter = ""), [library])
    for module in ctx.attrs.linked:
        wasm = module[DefaultInfo].default_outputs[0]
        add("--file=" + wasm.basename, [wasm])
    data = ctx.attrs.data or [None] * len(ctx.attrs.manifests)
    app_types = ctx.attrs.app_types or [None] * len(ctx.attrs.manifests)
    for manifest, module, files, types in zip(ctx.attrs.manifests, ctx.attrs.modules, data, app_types):
        if only_libraries:
            break
        if module:
            wasm = module[DefaultInfo].default_outputs[0]
            add(cmd_args(manifest, wasm.basename, delimiter = "="), [manifest, wasm])
        else:
            add(cmd_args(manifest, "", delimiter = "="), [manifest])
        if files:
            add(cmd_args("--data=", files, delimiter = ""), [files])
        for type_module in types[DefaultInfo].default_outputs if types else []:
            add(cmd_args("--app-type=", type_module, delimiter = ""), [type_module])
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
    packed = ctx.actions.declare_output(ctx.label.name + "-packed", dir = True)
    ctx.actions.dynamic_output(
        dynamic = [layout],
        f = lambda ctx, artifacts, outputs: _lay_out(ctx, artifacts[layout].read_json(), arguments, beside, generated, name, outputs[out], outputs[packed]),
        inputs = [],
        outputs = [out.as_output(), packed.as_output()],
    )
    archive = ctx.actions.declare_output(ctx.label.name + ".programz")
    ctx.actions.run(
        cmd_args(ctx.attrs._archiver[RunInfo], "pack", packed, archive.as_output()),
        category = "program_archive",
    )
    providers = [
        DefaultInfo(
            default_output = out,
            sub_targets = {"archive": [DefaultInfo(default_output = archive)]},
        )
    ]
    if name:
        besides = [cmd_args("--beside=", arguments[index][0], delimiter = "") for index in beside]
        providers.append(RunInfo(args = cmd_args(ctx.attrs._unpacker[RunInfo], "run", besides, archive, name)))
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
        # Each app's read-only data (app_data), in the order of manifests, or
        # None for an app staged without its module.
        "data": attrs.list(attrs.option(attrs.source()), default = []),
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
        # Each app's module, in the order of manifests, or None to stage the
        # manifest (and its type module) without it.
        "modules": attrs.list(attrs.option(attrs.dep())),
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

# Builds a dependency for the host on beui's HTML core (linux_x86_64_html).
def _html_host_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.linux_x86_64_html[PlatformInfo], refs)

_html_host_transition = transition(
    impl = _html_host_transition_impl,
    refs = {"linux_x86_64_html": "root//buck/platforms:linux_x86_64_html"} | PROFILE_REFS,
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
