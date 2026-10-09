# The rules that turn buck/tools/BUCK's downloads into tools, run on workers.

# The container a worker runs actions in: Ubuntu 24.04's buildpack-deps, pinned
# by digest. It brings glibc, libstdc++ and the Python the prelude runs on; the
# compilers come from buck/tools/BUCK. Namespace runs only its own copy of it
# in its registry (guides/build_server.md); the other servers pull it from
# Docker Hub. Hermetiq's Buildbarn picks no image per action: its scheduler
# sends an action to the pool whose platform is exactly the one asked for, and
# the pool below runs Ubuntu 24.04 (catthehacker/ubuntu:act-24.04).
# be3.build_server is the server ./scripts/buck builds on.
def worker_properties():
    server = read_root_config("be3", "build_server", "namespace")
    if server == "hermetiq":
        return {
            "env": "ubuntu2404-62d572b92f9f",
            "pool": "hmq-ci",
        }
    if server == "namespace":
        image = "docker://nscr.io/nmbprh983nhl8/be3-worker@sha256:a8f4627669b71081a3f3a0db26375e35fce20b75335130b50b8526bba1d0a497"
    else:
        image = "docker://docker.io/library/buildpack-deps@sha256:2607512c685336a441eba9719ab17da07137ab3178ae8b7118dfe1dff7991549"
    return {
        "OSFamily": "Linux",
        "container-image": image,
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
    ctx.actions.run(
        cmd_args("sh", ctx.attrs._script, out.as_output(), ctx.attrs.packages),
        category = "llvm_tree",
    )
    return [DefaultInfo(default_output = out)]

llvm_tree = rule(
    attrs = {
        "packages": attrs.list(attrs.source()),
        "_script": attrs.default_only(attrs.source(default = "root//buck/tools:llvm_tree.sh")),
    },
    impl = _llvm_tree_impl,
)

# A program from a Rust sysroot, run with SDKROOT set to Apple's SDK, as one
# executable rather than a shell command: the prelude's clippy wrapper writes
# each argument of the driver's command on a line of its own, so only a lone
# program survives it. SDKROOT must be absolute, and build scripts run the
# compiler from directories of their own, so the script finds the SDK and the
# program from where it is itself.
def _apple_sdk_tool_impl(ctx: AnalysisContext) -> list[Provider]:
    script = ctx.actions.declare_output("{}.sh".format(ctx.attrs.program))
    ctx.actions.write(
        script,
        [
            "#!/bin/sh",
            'here="$(cd "$(dirname "$0")" && pwd)"',
            cmd_args(
                cmd_args(ctx.attrs.sdk, format = 'SDKROOT="$here/{}"', relative_to = (script, 1)),
                "exec",
                cmd_args(ctx.attrs.sysroot, format = '"$here/{}/bin/' + ctx.attrs.program + '"', relative_to = (script, 1)),
                '"$@"',
                delimiter = " ",
            ),
        ],
        allow_args = True,
        is_executable = True,
    )
    return [
        DefaultInfo(default_output = script),
        RunInfo(args = cmd_args(script, hidden = [ctx.attrs.sdk, ctx.attrs.sysroot])),
    ]

apple_sdk_tool = rule(
    attrs = {
        "program": attrs.string(),
        "sdk": attrs.source(),
        "sysroot": attrs.source(),
    },
    impl = _apple_sdk_tool_impl,
)
