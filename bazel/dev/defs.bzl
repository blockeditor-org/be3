# rustfmt from the Rust toolchain, as a file a script on this machine runs:
# //:verify formats with the same rustfmt the toolchain pins.
def _rustfmt_impl(ctx):
    toolchain = ctx.toolchains["@rules_rust//rust/rustfmt:toolchain_type"]
    return [DefaultInfo(
        files = depset([toolchain.rustfmt]),
        runfiles = ctx.runfiles(transitive_files = toolchain.all_files),
    )]

rustfmt = rule(
    implementation = _rustfmt_impl,
    toolchains = ["@rules_rust//rust/rustfmt:toolchain_type"],
)
