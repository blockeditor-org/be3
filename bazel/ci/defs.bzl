# What CI builds, in one command: each target here names the platform and the
# profile it is for, so one command holds every platform and both profiles.
def _configured_transition_impl(_settings, attr):
    return {
        "//command_line_option:compilation_mode": "opt" if attr.release else "fastbuild",
        "//command_line_option:platforms": str(attr.platform),
    }

_configured_transition = transition(
    implementation = _configured_transition_impl,
    inputs = [],
    outputs = [
        "//command_line_option:compilation_mode",
        "//command_line_option:platforms",
    ],
)

def _configured_impl(ctx):
    return [DefaultInfo(files = ctx.attr.actual[0][DefaultInfo].files)]

# actual, built for platform, in cargo's release profile with release.
configured = rule(
    implementation = _configured_impl,
    attrs = {
        "actual": attr.label(cfg = _configured_transition),
        "platform": attr.label(),
        "release": attr.bool(),
    },
)

# What CI uploads, as one directory: each artifact's output at the path its
# upload names.
def _bundle_impl(ctx):
    out = ctx.actions.declare_directory(ctx.label.name)
    inputs = []
    arguments = [out.path]
    for target, path in ctx.attr.contents.items():
        files = target[DefaultInfo].files.to_list()
        if len(files) != 1:
            fail("{} is not one file or directory".format(target.label))
        inputs.append(files[0])
        arguments.extend([files[0].path, path])
    ctx.actions.run_shell(
        outputs = [out],
        inputs = inputs,
        command = """
set -eu
out="$1"
shift
while [ $# -gt 0 ]; do
    mkdir -p "$out/$(dirname "$2")"
    cp -RL "$1" "$out/$2"
    shift 2
done
""",
        arguments = arguments,
        mnemonic = "Bundle",
    )
    return [DefaultInfo(files = depset([out]))]

bundle = rule(
    implementation = _bundle_impl,
    attrs = {"contents": attr.label_keyed_string_dict()},
)
