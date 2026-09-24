# A tool built on a worker with `cargo install --locked`: reindeer and
# rust-project from pinned commits, wasm-bindgen from a pinned version. The pin
# and the toolchain are the inputs, so it is built once and cached.
def _cargo_install_impl(ctx: AnalysisContext) -> list[Provider]:
    binary = ctx.attrs.binary or ctx.attrs.package
    out = ctx.actions.declare_output(binary)
    if ctx.attrs.version:
        source = ["--version", ctx.attrs.version]
    else:
        source = ["--git", ctx.attrs.repository, "--rev", ctx.attrs.revision]
    script = """
set -eu
out="$1"
toolchain="$(cd "$2" && pwd)"
package="$3"
binary="$4"
shift 4
scratch="$(mktemp -d)"
export PATH="$toolchain/bin:$PATH"
export CARGO_HOME="$scratch/cargo-home"
export CARGO_TARGET_DIR="$scratch/target"
cargo install --quiet --locked --root "$scratch/root" "$@" "$package"
cp "$scratch/root/bin/$binary" "$out"
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
            ctx.attrs.package,
            binary,
            source,
        ),
        category = "cargo_install",
    )
    return [DefaultInfo(default_output = out), RunInfo(args = cmd_args(out))]

cargo_install = rule(
    attrs = {
        "binary": attrs.option(attrs.string(), default = None),
        "package": attrs.string(),
        "repository": attrs.option(attrs.string(), default = None),
        "revision": attrs.option(attrs.string(), default = None),
        "toolchain": attrs.source(),
        "version": attrs.option(attrs.string(), default = None),
    },
    impl = _cargo_install_impl,
)
