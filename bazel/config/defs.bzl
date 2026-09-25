load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")

# The commit a release build reports, as the environment file rustc reads it
# from: `--//bazel/config:commit=SHA`, since a worker has no checkout to ask
# git. A dev build says unknown whatever it is passed, so the file, and so what
# is compiled from it, is the same for every commit.
def _commit_env_impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".env")
    commit = "unknown"
    if ctx.var["COMPILATION_MODE"] == "opt":
        commit = ctx.attr._commit[BuildSettingInfo].value
    ctx.actions.write(out, "{}={}\n".format(ctx.attr.variable, commit))
    return [DefaultInfo(files = depset([out]))]

commit_env = rule(
    implementation = _commit_env_impl,
    attrs = {
        "variable": attr.string(mandatory = True),
        "_commit": attr.label(default = "//bazel/config:commit"),
    },
)
