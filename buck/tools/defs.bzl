# The rules that turn buck/tools/BUCK's downloads into tools, run on workers.

# The container a worker runs actions in: Ubuntu 24.04's buildpack-deps, pinned
# by digest. It brings glibc, libstdc++ and the Python the prelude runs on; the
# compilers come from buck/tools/BUCK.
worker_properties = {
    "OSFamily": "Linux",
    "container-image": "docker://docker.io/library/buildpack-deps@sha256:2607512c685336a441eba9719ab17da07137ab3178ae8b7118dfe1dff7991549",
}

# Lays the Rust dist components over one another into the sysroot rustup would
# have installed: rustc's bin and lib, each target's standard library under
# lib/rustlib, and clippy-driver beside rustc so that it finds the same
# librustc_driver through the same $ORIGIN/../lib.
def _rust_sysroot_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output("sysroot", dir = True)
    ctx.actions.run(
        cmd_args(
            "sh",
            "-c",
            'out="$1"; shift; mkdir -p "$out"; for component; do cp -R "$component"/. "$out"/; done',
            "--",
            out.as_output(),
            ctx.attrs.components,
        ),
        category = "rust_sysroot",
    )
    return [DefaultInfo(default_output = out)]

rust_sysroot = rule(
    attrs = {"components": attrs.list(attrs.source())},
    impl = _rust_sysroot_impl,
)

# Unpacks Ubuntu's clang packages into a self-contained llvm-20 tree: the
# binaries, the libraries they load through $ORIGIN/../lib, clang's resource
# directory and libclang for bindgen. Symlinks within bin stay, since a driver's
# name decides how it behaves.
def _llvm_tree_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output("llvm", dir = True)
    script = """
set -eu
out="$1"
shift
packages="$(mktemp -d)"
for package; do dpkg-deb -x "$package" "$packages"; done
mkdir -p "$out/bin" "$out/lib"
cp -P "$packages"/usr/lib/llvm-20/bin/clang "$packages"/usr/lib/llvm-20/bin/clang++ \
    "$packages"/usr/lib/llvm-20/bin/lld "$packages"/usr/lib/llvm-20/bin/ld.lld \
    "$packages"/usr/lib/llvm-20/bin/ld64.lld "$packages"/usr/lib/llvm-20/bin/lld-link \
    "$packages"/usr/lib/llvm-20/bin/llvm-ar "$packages"/usr/lib/llvm-20/bin/llvm-lib "$out/bin/"
cp "$packages"/usr/lib/x86_64-linux-gnu/libLLVM.so.20.1 \
    "$packages"/usr/lib/x86_64-linux-gnu/libclang-cpp.so.20.1 "$out/lib/"
cp "$packages"/usr/lib/x86_64-linux-gnu/libclang-20.so.20 "$out/lib/libclang.so"
cp -R "$packages"/usr/lib/llvm-20/lib/clang "$out/lib/"
rm -rf "$packages"
"""
    ctx.actions.run(
        cmd_args("sh", "-c", script, "--", out.as_output(), ctx.attrs.packages),
        category = "llvm_tree",
    )
    return [DefaultInfo(default_output = out)]

llvm_tree = rule(
    attrs = {"packages": attrs.list(attrs.source())},
    impl = _llvm_tree_impl,
)

# A command made of artifacts and arguments, as a tool a toolchain can name.
# Its location macros are what put the directories above into every action
# that runs it, which is how a worker comes to have the compiler at all.
def _tool_impl(ctx: AnalysisContext) -> list[Provider]:
    return [
        DefaultInfo(),
        RunInfo(args = cmd_args(ctx.attrs.command)),
    ]

tool = rule(
    attrs = {"command": attrs.list(attrs.arg())},
    impl = _tool_impl,
)
