# The benchmark suite as one directory (guides/performance.md): the runner as
# `bench`, each benchmark under programs/ by name, and block-app with every
# plugin, and the libraries it loads, for the end-to-end benchmarks. `buck2
# build --out DIR` keeps a build of it to compare another against.
def _suite_impl(ctx: AnalysisContext) -> list[Provider]:
    contents = {
        "bench": ctx.attrs.runner[DefaultInfo].default_outputs[0],
        "block-app": ctx.attrs.app[DefaultInfo].default_outputs[0],
        "lib": ctx.attrs.libraries[DefaultInfo].default_outputs[0],
    }
    for name, program in ctx.attrs.programs.items():
        contents["programs/" + name] = program[DefaultInfo].default_outputs[0]
    out = ctx.actions.copied_dir(ctx.label.name, contents)
    return [DefaultInfo(default_output = out), RunInfo(args = cmd_args(out.project("bench")))]

suite = rule(
    attrs = {
        "app": attrs.dep(),
        "libraries": attrs.dep(),
        "programs": attrs.dict(attrs.string(), attrs.dep()),
        "runner": attrs.dep(),
    },
    impl = _suite_impl,
)
