def _signer_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output("apksign.wasm")
    ctx.actions.run(
        cmd_args(
            "sh",
            ctx.attrs._build,
            ctx.attrs.go,
            ctx.attrs.source,
            ctx.attrs.keystore,
            ctx.attrs.pkcs12,
            ctx.attrs.crypto,
            out.as_output(),
            ctx.attrs._fix,
        ),
        category = "apksign_wasm",
    )
    return [DefaultInfo(default_output = out)]

wasm_signer = rule(
    attrs = {
        "crypto": attrs.source(),
        "go": attrs.source(),
        "keystore": attrs.source(),
        "pkcs12": attrs.source(),
        "source": attrs.source(),
        "_build": attrs.default_only(attrs.source(default = "root//buck/android:build-sign-wasm.sh")),
        "_fix": attrs.default_only(attrs.source(default = "root//buck/android:sign-wasm.patch")),
    },
    impl = _signer_impl,
)

def _trial_impl(ctx: AnalysisContext) -> list[Provider]:
    return [
        DefaultInfo(),
        RunInfo(args = cmd_args(ctx.attrs.tool[RunInfo], "apk-sign", ctx.attrs.signer)),
    ]

signing_trial = rule(
    attrs = {
        "signer": attrs.source(),
        "tool": attrs.dep(providers = [RunInfo]),
    },
    impl = _trial_impl,
)
