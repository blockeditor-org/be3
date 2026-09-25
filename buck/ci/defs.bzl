# What CI uploads, as one directory: each artifact's output at the path its
# upload names. The copies are buck2's own, so a build that downloads nothing
# makes none of them.
def _bundle_impl(ctx: AnalysisContext) -> list[Provider]:
    contents = {}
    for path, dep in ctx.attrs.contents.items():
        contents[path] = dep[DefaultInfo].default_outputs[0]
    return [DefaultInfo(default_output = ctx.actions.copied_dir(ctx.label.name, contents))]

bundle = rule(
    attrs = {"contents": attrs.dict(attrs.string(), attrs.dep())},
    impl = _bundle_impl,
)

# Everything its deps build, with no output of its own for a build to fetch.
def _group_impl(ctx: AnalysisContext) -> list[Provider]:
    outputs = []
    for dep in ctx.attrs.deps:
        outputs.extend(dep[DefaultInfo].default_outputs)
        outputs.extend(dep[DefaultInfo].other_outputs)
    return [DefaultInfo(other_outputs = outputs)]

group = rule(
    attrs = {"deps": attrs.list(attrs.dep())},
    impl = _group_impl,
)
