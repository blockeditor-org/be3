load("@root//buck/platforms:cross.bzl", "CROSS_PLATFORMS", "cross_triple", "plan_name")
load("@root//third-party/rust:downloads.bzl", "downloads")
load(":inputs.bzl", "CargoInputsInfo")

# The crates' facts generated from cargo's plans on a worker, as a directory
# beside the Cargo.lock they were planned with: //:cargo-plan, which
# ./scripts/verify builds (buck/dev/verify.bxl), and buckify.bxl, which
# ./scripts/buckify runs, since only BXL can find a crate the members index
# does not name yet. crates/be-tool-verify copies the files into place.
#
# The action is crates/be-tool-cargo-offline: it copies the workspace's
# manifests and Cargo.lock, with empty files at the paths cargo discovers
# targets at (inputs.bzl), and hands cargo the crates the checked-in
# third-party/rust/downloads.bzl pins (vendored), which buck2 keeps across
# daemons, as a vendored directory, with no network. Then
# crates/be-tool-buck's plan asks cargo for its plans and generates the facts
# from them. When the lockfile needs a crate the downloads do not have, the
# action says so in OUT/stale: the bootstrap's lock step pins it before buck2
# runs (crates/be-tool-bootstrap/src/lock.rs).
#
# be-tool-buck is built by buck2 from the checked-in facts. When those cannot
# build it, because its own dependencies changed, ./scripts/buckify plans with
# one cargo builds inside the action instead (cargo_built in buckify.bxl).

# The crates buckify's fallback builds be-tool-buck from with cargo, whose
# sources are its inputs then.
TOOLS = ["crates/be-tool-buck", "crates/be-lib-crate-pins"]

# Every crate the checked-in downloads pin, as the targets that unpack them
# (third_party.bzl), by key.
VENDORED = {key: "root//third-party/rust:{}.crate".format(key) for key in downloads}

# The action: real is the workspace's files by path, placeholders the paths
# that are only discovered, vendored each download's unpacked directory by
# key, and tools be-tool-buck's executable, or None to build it with cargo.
def buckify_action(actions, offline, real: dict, placeholders: list[str], cargo: Artifact, nightly: Artifact, tools, vendored: dict) -> Artifact:
    workspace = actions.copied_dir("workspace", real)
    layout = actions.write("placeholders.txt", sorted(placeholders))
    out = actions.declare_output("generated", dir = True)
    actions.run(
        cmd_args(
            offline,
            out.as_output(),
            workspace,
            layout,
            cargo,
            nightly,
            ["--tools", tools] if tools else ["--cargo-build"],
            ["{}={}".format(plan_name(name), cross_triple(name)) for name in CROSS_PLATFORMS],
            "--",
            [cmd_args(key, downloads[key]["sha256"], vendored[key], delimiter = "=") for key in sorted(vendored)],
        ),
        category = "buckify",
    )
    return out

def _cargo_plan_impl(ctx: AnalysisContext) -> list[Provider]:
    real = {"Cargo.lock": ctx.attrs.lock, "Cargo.toml": ctx.attrs.manifest}
    placeholders = []
    for crate in ctx.attrs.crates:
        info = crate[CargoInputsInfo]
        real[crate.label.package + "/Cargo.toml"] = info.manifest
        placeholders.extend(info.discovered)
    out = buckify_action(
        ctx.actions,
        ctx.attrs._offline[RunInfo],
        real,
        placeholders,
        ctx.attrs._cargo[DefaultInfo].default_outputs[0],
        ctx.attrs._nightly[DefaultInfo].default_outputs[0],
        ctx.attrs._tools[RunInfo],
        {key: dep[DefaultInfo].default_outputs[0] for key, dep in ctx.attrs.vendored.items()},
    )
    return [DefaultInfo(default_output = out)]

# The action over the crates the members index names (each one's
# :cargo-inputs), with be-tool-buck built by buck2.
cargo_plan = rule(
    attrs = {
        "crates": attrs.list(attrs.dep(providers = [CargoInputsInfo])),
        "lock": attrs.source(),
        "manifest": attrs.source(),
        "vendored": attrs.dict(default = VENDORED, key = attrs.string(), value = attrs.dep()),
        "_cargo": attrs.default_only(attrs.dep(default = "root//buck/cargo:cargo")),
        "_nightly": attrs.default_only(attrs.dep(default = "root//buck/cargo:nightly")),
        "_offline": attrs.default_only(attrs.exec_dep(default = "root//crates/be-tool-cargo-offline:bin", providers = [RunInfo])),
        "_tools": attrs.default_only(attrs.exec_dep(default = "root//crates/be-tool-buck:bin", providers = [RunInfo])),
    },
    impl = _cargo_plan_impl,
)
