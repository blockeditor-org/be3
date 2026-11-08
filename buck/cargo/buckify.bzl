load("@root//buck/platforms:cross.bzl", "CROSS_PLATFORMS", "cross_triple", "plan_name")
load(":crates.bzl", "downloads")

# buck/cargo/crates.bzl generated (crates/be-tool-buck's generate, from cargo's
# plans) on a worker, as a directory beside the Cargo.lock it was generated
# from: //:cargo-plan, which ./scripts/verify builds through //:verify, and
# buckify.bxl, which //:buckify (update.sh) runs, since only BXL can find a
# crate crates.bzl does not name yet. Each copies it into place.
#
# The action's inputs are the manifests, Cargo.lock and buck-tools' sources,
# plus empty files at the paths cargo discovers targets at (DISCOVERED).
# Editing or adding an ordinary source file leaves its key, and the cache entry
# everyone shares, alone.
#
# The crates are the downloads the checked-in crates.bzl pins (vendored),
# which buck2 keeps across daemons: cargo reads them as a vendored directory,
# and the action has no network. When the lockfile needs a crate they do not
# have, the action says so in OUT/stale (buckify.sh): the bootstrap's lock step
# pins it before buck2 runs (crates/be-tool-bootstrap/src/lock.rs).

# The files below a crate cargo looks for when discovering its targets. Paths
# a Cargo.toml names are recreated by the script, which can read it.
DISCOVERED = ["build.rs", "src/lib.rs", "src/main.rs", "src/bin/**", "examples/**", "benches/**", "tests/*", "tests/**/main.rs"]

# The crates the action builds buck-tools from, whose sources are its inputs.
TOOLS = ["crates/be-tool-buck", "crates/be-lib-crate-pins"]

# Every crate the checked-in crates.bzl downloads, as the targets that unpack
# them (third_party.bzl), by key.
VENDORED = {key: "root//third-party/rust:{}.crate".format(key) for key in downloads}

# The action: real is the workspace's files by path, placeholders the paths
# that are only discovered, and vendored each download's unpacked directory by
# key.
def buckify_action(actions, real: dict, placeholders: list[str], cargo: Artifact, nightly: Artifact, script, vendored: dict) -> Artifact:
    workspace = actions.copied_dir("workspace", real)
    layout = actions.write("placeholders.txt", sorted(placeholders))
    out = actions.declare_output("generated", dir = True)
    actions.run(
        cmd_args(
            "sh",
            script,
            out.as_output(),
            workspace,
            layout,
            cargo,
            nightly,
            ["{}={}".format(plan_name(name), cross_triple(name)) for name in CROSS_PLATFORMS],
            "--",
            [cmd_args(key, downloads[key]["sha256"], str(downloads[key]["size_bytes"]), vendored[key], delimiter = "=") for key in sorted(vendored)],
        ),
        category = "buckify",
    )
    return out

# What one crate gives the action: its Cargo.toml, the paths under it cargo
# discovers, and for TOOLS its sources, every path from the repository's root.
CargoInputsInfo = provider(fields = {"discovered": list[str], "manifest": Artifact, "sources": dict})

def _cargo_inputs_impl(ctx: AnalysisContext) -> list[Provider]:
    prefix = ctx.label.package + "/"
    return [
        DefaultInfo(),
        CargoInputsInfo(
            discovered = [prefix + path for path in ctx.attrs.discovered],
            manifest = ctx.attrs.manifest,
            sources = {prefix + path: source for path, source in ctx.attrs.sources.items()},
        ),
    ]

_cargo_inputs = rule(
    attrs = {
        "discovered": attrs.list(attrs.string()),
        "manifest": attrs.source(),
        "sources": attrs.dict(key = attrs.string(), value = attrs.source()),
    },
    impl = _cargo_inputs_impl,
)

# A crate's :cargo-inputs, once: buck/cargo/defs.bzl's macros define it. The
# discovered paths are strings, so that their contents are no input.
def cargo_inputs():
    if native.rule_exists("cargo-inputs"):
        return
    sources = {}
    if native.package_name() in TOOLS:
        sources = {path: path for path in native.glob(["src/**"])}
    _cargo_inputs(
        name = "cargo-inputs",
        discovered = [path for path in native.glob(DISCOVERED) if path not in sources],
        manifest = "Cargo.toml",
        sources = sources,
        visibility = ["root//:"],
    )

def _cargo_plan_impl(ctx: AnalysisContext) -> list[Provider]:
    real = {"Cargo.lock": ctx.attrs.lock, "Cargo.toml": ctx.attrs.manifest}
    placeholders = []
    for crate in ctx.attrs.crates:
        info = crate[CargoInputsInfo]
        real[crate.label.package + "/Cargo.toml"] = info.manifest
        real.update(info.sources)
        placeholders.extend(info.discovered)
    out = buckify_action(
        ctx.actions,
        real,
        placeholders,
        ctx.attrs._cargo[DefaultInfo].default_outputs[0],
        ctx.attrs._nightly[DefaultInfo].default_outputs[0],
        ctx.attrs._script,
        {key: dep[DefaultInfo].default_outputs[0] for key, dep in ctx.attrs.vendored.items()},
    )
    return [DefaultInfo(default_output = out)]

# The action over the crates crates.bzl names (each one's :cargo-inputs).
cargo_plan = rule(
    attrs = {
        "crates": attrs.list(attrs.dep(providers = [CargoInputsInfo])),
        "lock": attrs.source(),
        "manifest": attrs.source(),
        "vendored": attrs.dict(default = VENDORED, key = attrs.string(), value = attrs.dep()),
        "_cargo": attrs.default_only(attrs.dep(default = "root//buck/cargo:cargo")),
        "_nightly": attrs.default_only(attrs.dep(default = "root//buck/cargo:nightly")),
        "_script": attrs.default_only(attrs.source(default = "root//buck/cargo:buckify.sh")),
    },
    impl = _cargo_plan_impl,
)
