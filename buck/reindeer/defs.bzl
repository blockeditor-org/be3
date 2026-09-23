# Builds reindeer on a worker with `cargo install`, the way a developer used to
# on their own machine.
#
# reindeer publishes no releases, so it is built from a pinned commit, with the
# nightly its own rust-toolchain asks for. cargo fetches the commit and
# reindeer's crates itself, which needs the network; the revision and the
# toolchain are the inputs, so the result is cached like any other action and
# built once rather than once per machine.
def _reindeer_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output("reindeer")
    script = """
set -eu
out="$1"
toolchain="$(cd "$2" && pwd)"
revision="$3"
scratch="$(mktemp -d)"
export PATH="$toolchain/bin:$PATH"
export CARGO_HOME="$scratch/cargo-home"
export CARGO_TARGET_DIR="$scratch/target"
cargo install --quiet --locked --root "$scratch/root" \
    --git https://github.com/facebookincubator/reindeer.git --rev "$revision" reindeer
cp "$scratch/root/bin/reindeer" "$out"
rm -rf "$scratch"
"""
    ctx.actions.run(
        cmd_args("sh", "-c", script, "--", out.as_output(), ctx.attrs.toolchain, ctx.attrs.revision),
        category = "cargo_install",
    )
    return [DefaultInfo(default_output = out), RunInfo(args = cmd_args(out))]

reindeer = rule(
    attrs = {
        "revision": attrs.string(),
        "toolchain": attrs.source(),
    },
    impl = _reindeer_impl,
)
