# The app as it runs (bazel/app/stage.sh): the executable under cargo's name,
# and beside it every plugin, each module precompiled for precompile_target in
# an action of its own by plugin-test-runner, block-wasm-host's own engine,
# which runs on the workers whatever the app is built for. With no binary it is
# the plugins alone. The web bundle is this rule too: bindgen runs wasm-bindgen
# over each module, and index writes the plugins.json a browser finds the
# plugins through.
def _only(target, extension = None):
    files = target.files.to_list()
    if extension:
        files = [file for file in files if file.extension == extension]
    if len(files) != 1:
        fail("{} is not one file".format(target.label))
    return files[0]

def _app_impl(ctx):
    out = ctx.actions.declare_directory(ctx.attr.out or ctx.label.name)
    arguments = ctx.actions.args()
    arguments.add(ctx.file._stage)
    arguments.add(out.path)
    inputs = []
    name = None
    if ctx.attr.binary:
        executable = ctx.executable.binary
        name = ctx.attr.executable_name + ("." + executable.extension if executable.extension else "")
        arguments.add("--executable={}={}".format(executable.path, name))
        inputs.append(executable)
    for extra, extra_name in ctx.attr.extra_binaries.items():
        file = extra[DefaultInfo].files_to_run.executable
        arguments.add("--executable={}={}".format(file.path, extra_name + ("." + file.extension if file.extension else "")))
        inputs.append(file)
    for file in ctx.files.files:
        arguments.add("--file=" + file.path)
        inputs.append(file)
    if len(ctx.attr.manifests) != len(ctx.attr.modules):
        fail("every manifest needs its module")
    for manifest, module in zip(ctx.files.manifests, ctx.attr.modules):
        wasm = _only(module, "wasm")
        arguments.add("{}={}".format(manifest.path, wasm.path))
        inputs.extend([manifest, wasm])
        if ctx.attr.precompile:
            # One action a module, so that the thirty-odd compiles run side by
            # side on as many workers rather than one after another.
            artifact = ctx.actions.declare_file("{}.cwasm/{}.cwasm".format(ctx.label.name, wasm.basename.removesuffix(".wasm")))
            ctx.actions.run(
                outputs = [artifact],
                inputs = [wasm],
                executable = ctx.executable._precompiler,
                arguments = ["--precompile-to", "--target", ctx.attr.precompile_target, artifact.path, wasm.path],
                mnemonic = "WasmPrecompile",
                progress_message = "Precompiling %{input} for " + ctx.attr.precompile_target,
            )
            arguments.add("--artifact=" + artifact.path)
            inputs.append(artifact)
    for module in ctx.attr.bindgen:
        wasm = _only(module, "wasm")
        bindings = ctx.actions.declare_directory("{}.bindings/{}".format(ctx.label.name, wasm.basename.removesuffix(".wasm")))
        ctx.actions.run(
            outputs = [bindings],
            inputs = [wasm],
            executable = ctx.executable._wasm_bindgen,
            arguments = ["--target", "web", "--no-typescript", "--out-dir", bindings.path, wasm.path],
            mnemonic = "WasmBindgen",
            progress_message = "Running wasm-bindgen over %{input}",
        )
        arguments.add("--tree=" + bindings.path)
        inputs.append(bindings)
    if ctx.attr.index:
        arguments.add("--index")
    ctx.actions.run(
        outputs = [out],
        inputs = inputs + [ctx.file._stage],
        executable = "/bin/sh",
        arguments = [arguments],
        mnemonic = "StageApp",
        progress_message = "Staging %{label}",
    )
    if not name:
        return [DefaultInfo(files = depset([out]))]
    launcher = ctx.actions.declare_file(ctx.label.name + ".run")
    ctx.actions.write(
        launcher,
        '#!/bin/sh\nexec "$(dirname "$0")/{}/{}" "$@"\n'.format(ctx.attr.out or ctx.label.name, name),
        is_executable = True,
    )
    return [DefaultInfo(
        executable = launcher,
        files = depset([out]),
        runfiles = ctx.runfiles([out]),
    )]

_ATTRS = {
    "binary": attr.label(executable = True, cfg = "target"),
    "bindgen": attr.label_list(),
    "executable_name": attr.string(),
    # More executables to put beside the app, by the name cargo gives each.
    "extra_binaries": attr.label_keyed_string_dict(cfg = "target"),
    "files": attr.label_list(allow_files = True),
    "index": attr.bool(),
    "manifests": attr.label_list(allow_files = [".json"]),
    "modules": attr.label_list(),
    # The directory's name, when the target's is taken.
    "out": attr.string(),
    "precompile": attr.bool(default = True),
    "precompile_target": attr.string(default = "x86_64-unknown-linux-gnu"),
    "_precompiler": attr.label(default = "//crates/plugin-test-runner:plugin-test-runner-bin", executable = True, cfg = "exec"),
    "_stage": attr.label(default = "//bazel/app:stage.sh", allow_single_file = True),
    "_wasm_bindgen": attr.label(default = "//bazel/cargo:wasm-bindgen", executable = True, cfg = "exec"),
}

app = rule(implementation = _app_impl, attrs = _ATTRS, executable = True)

# The same, for a layout nothing runs directly: the plugins, the web bundle,
# what ships.
app_files = rule(implementation = _app_impl, attrs = _ATTRS)

# One file out of a directory fetch made (bazel/tools/defs.bzl), such as a
# library out of a release archive.
def _tree_file_impl(ctx):
    out = ctx.actions.declare_file(ctx.attr.out or ctx.attr.path.rsplit("/", 1)[-1])
    ctx.actions.run_shell(
        outputs = [out],
        inputs = [ctx.file.tree],
        command = 'cp "$1/$2" "$3"',
        arguments = [ctx.file.tree.path, ctx.attr.path, out.path],
        mnemonic = "CopyFromTree",
    )
    return [DefaultInfo(files = depset([out]))]

tree_file = rule(
    implementation = _tree_file_impl,
    attrs = {
        "out": attr.string(),
        "path": attr.string(mandatory = True),
        "tree": attr.label(allow_single_file = True),
    },
)

# A shell script as a command `./scripts/bazel run` runs, with the target's
# args (location macros expanded against data) before the ones the command
# line adds. It runs the script with sh, since a file a build reads has no
# executable bit (//:verify says why).
def _sh_command_impl(ctx):
    launcher = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(
        launcher,
        "#!/bin/sh\nexec /bin/sh {} \"$@\"\n".format(_shell_quote(ctx.file.src.short_path)),
        is_executable = True,
    )
    runfiles = ctx.runfiles([ctx.file.src] + ctx.files.data)
    for target in ctx.attr.data:
        runfiles = runfiles.merge(target[DefaultInfo].default_runfiles)
    return [DefaultInfo(executable = launcher, runfiles = runfiles)]

def _shell_quote(text):
    return "'" + text.replace("'", "'\\''") + "'"

sh_command = rule(
    implementation = _sh_command_impl,
    attrs = {
        "data": attr.label_list(allow_files = True),
        "src": attr.label(allow_single_file = True, mandatory = True),
    },
    executable = True,
)
