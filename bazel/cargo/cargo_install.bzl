# A tool built on a worker with `cargo install --locked`, from a pinned version,
# with the workspace's own Rust. The pin and the toolchain are the inputs, so
# it is built once and cached.
def _cargo_install_impl(ctx):
    toolchain = ctx.attr._toolchain[platform_common.ToolchainInfo]
    out = ctx.actions.declare_file(ctx.attr.binary or ctx.attr.package)
    ctx.actions.run_shell(
        outputs = [out],
        inputs = ctx.attr._toolchain[DefaultInfo].files,
        command = """
set -eu
out="$PWD/$1"
cargo="$PWD/$2"
rustc="$PWD/$3"
package="$4"
binary="$5"
version="$6"
scratch="$(mktemp -d)"
export CARGO_HOME="$scratch/cargo-home"
export CARGO_TARGET_DIR="$scratch/target"
export RUSTC="$rustc"
"$cargo" install --quiet --locked --root "$scratch/root" --version "$version" "$package"
cp "$scratch/root/bin/$binary" "$out"
rm -rf "$scratch"
""",
        arguments = [
            out.path,
            toolchain.cargo.path,
            toolchain.rustc.path,
            ctx.attr.package,
            ctx.attr.binary or ctx.attr.package,
            ctx.attr.version,
        ],
        mnemonic = "CargoInstall",
        progress_message = "Installing %{label} with cargo",
        execution_requirements = {"requires-network": "1"},
    )
    return [DefaultInfo(executable = out, files = depset([out]))]

cargo_install = rule(
    implementation = _cargo_install_impl,
    attrs = {
        "binary": attr.string(),
        "package": attr.string(mandatory = True),
        "version": attr.string(mandatory = True),
        "_toolchain": attr.label(default = "@rules_rust//rust/toolchain:current_rust_toolchain", cfg = "exec"),
    },
    executable = True,
)
