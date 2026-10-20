def _android_apk_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.attrs.template[DefaultInfo].default_outputs[0]
    run = cmd_args(
        ctx.attrs._local[RunInfo],
        "apk-sign",
        "--in",
        out,
        "--out",
        "target/android/" + ctx.label.package.split("/")[-1] + ".apk",
        "--build-info",
        ctx.actions.copy_file("build-info.json", ctx.attrs._metadata),
        "--activity",
        ctx.attrs.application_id + "/" + ctx.attrs.activity,
    )
    if ctx.attrs.signing != "dev":
        run.add("--signing", ctx.attrs.signing)
    return [DefaultInfo(default_output = out), RunInfo(args = run)]

android_apk = rule(
    attrs = {
        "activity": attrs.string(),
        "application_id": attrs.string(),
        "signing": attrs.enum(["dev", "ci", "release"], default = "dev"),
        "template": attrs.dep(),
        "_local": attrs.default_only(attrs.dep(default = "root//crates/local-tools:bin", providers = [RunInfo])),
        "_metadata": attrs.default_only(attrs.source(default = "root//buck/app:build-info.json")),
    },
    impl = _android_apk_impl,
)
