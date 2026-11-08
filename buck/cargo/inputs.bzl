# What one crate gives buckify's action (buckify.bzl): its Cargo.toml, and the
# paths under it cargo discovers targets at, as strings, so that their contents
# are no input. Editing or adding an ordinary source file leaves the action's
# key, and the cache entry everyone shares, alone.

# The files below a crate cargo looks for when discovering its targets. Paths
# a Cargo.toml names are recreated by the action, which can read it.
DISCOVERED = ["build.rs", "src/lib.rs", "src/main.rs", "src/bin/**", "examples/**", "benches/**", "tests/*", "tests/**/main.rs"]

CargoInputsInfo = provider(fields = {"discovered": list[str], "manifest": Artifact})

def _cargo_inputs_impl(ctx: AnalysisContext) -> list[Provider]:
    prefix = ctx.label.package + "/"
    return [
        DefaultInfo(),
        CargoInputsInfo(
            discovered = [prefix + path for path in ctx.attrs.discovered],
            manifest = ctx.attrs.manifest,
        ),
    ]

_cargo_inputs = rule(
    attrs = {
        "discovered": attrs.list(attrs.string()),
        "manifest": attrs.source(),
    },
    impl = _cargo_inputs_impl,
)

# A crate's :cargo-inputs, which cargo_package() in defs.bzl defines.
def cargo_inputs():
    _cargo_inputs(
        name = "cargo-inputs",
        discovered = native.glob(DISCOVERED),
        manifest = "Cargo.toml",
        visibility = ["root//:"],
    )
