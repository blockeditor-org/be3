# A tool built on a worker with `cargo install --git`, the way a developer
# would on their own machine: reindeer, and buck2's rust-project.
#
# Neither publishes releases, so each is built from a pinned commit, with the
# pinned nightly. cargo fetches the commit and the tool's crates itself, which
# needs the network; the repository, the revision and the toolchain are the
# inputs, so the result is cached like any other action and built once rather
# than once per machine.
def _cargo_install_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output(ctx.attrs.package)
    script = """
set -eu
out="$1"
toolchain="$(cd "$2" && pwd)"
revision="$3"
repository="$4"
package="$5"
scratch="$(mktemp -d)"
export PATH="$toolchain/bin:$PATH"
export CARGO_HOME="$scratch/cargo-home"
export CARGO_TARGET_DIR="$scratch/target"
cargo install --quiet --locked --root "$scratch/root" \
    --git "$repository" --rev "$revision" "$package"
cp "$scratch/root/bin/$package" "$out"
rm -rf "$scratch"
"""
    ctx.actions.run(
        cmd_args(
            "sh",
            "-c",
            script,
            "--",
            out.as_output(),
            ctx.attrs.toolchain,
            ctx.attrs.revision,
            ctx.attrs.repository,
            ctx.attrs.package,
        ),
        category = "cargo_install",
    )
    return [DefaultInfo(default_output = out), RunInfo(args = cmd_args(out))]

cargo_install = rule(
    attrs = {
        "package": attrs.string(),
        "repository": attrs.string(),
        "revision": attrs.string(),
        "toolchain": attrs.source(),
    },
    impl = _cargo_install_impl,
)
