# The rules of what ./scripts/verify and ./scripts/ci build besides the tests
# (buck/dev/test.bzl): each package's autofixes and the bootstrap's pins.
# buck/dev/verify.bxl finds every one by kind; each builds alone too:
# `./scripts/buck build //crates/be-lib-geometry:fix`.

# The autofixes of one package, as crates/be-tool-buck's fix: srcs, by their
# paths in the repository, copied, fixed and compared with what they were.
# fix-rust-source and rustfmt fix the Rust files, starlark_fmt the BUCK,
# PACKAGE, .bzl and .bxl files.
def _source_fix_impl(ctx: AnalysisContext) -> list[Provider]:
    sources = ctx.actions.copied_dir("sources", ctx.attrs.srcs)
    out = ctx.actions.declare_output("fix", dir = True)
    rust = [path for path in ctx.attrs.srcs if path.endswith(".rs")]
    ctx.actions.run(
        cmd_args(
            ctx.attrs._tools[RunInfo],
            "fix",
            out.as_output(),
            sources,
            ["--fix-rust-source", ctx.attrs._fix_rust_source[RunInfo], "--rustfmt", ctx.attrs._rustfmt[DefaultInfo].default_outputs[0]] if rust else [],
            "--starlark-fmt",
            ctx.attrs._starlark_fmt[DefaultInfo].default_outputs[0],
            "--starlark-config",
            ctx.attrs._starlark_config,
        ),
        category = "fix",
        identifier = ctx.label.package or "root",
    )
    return [DefaultInfo(default_output = out)]

source_fix = rule(
    attrs = {
        "srcs": attrs.dict(key = attrs.string(), value = attrs.source()),
        "_fix_rust_source": attrs.default_only(attrs.exec_dep(default = "root//crates/be-tool-fix-rust-source:bin", providers = [RunInfo])),
        "_rustfmt": attrs.default_only(attrs.exec_dep(default = "root//buck/tools:rustfmt-sysroot")),
        "_starlark_config": attrs.default_only(attrs.source(default = "root//:starlark_fmt.json")),
        "_starlark_fmt": attrs.default_only(attrs.exec_dep(default = "root//buck/tools:starlark_fmt")),
        "_tools": attrs.default_only(attrs.exec_dep(default = "root//crates/be-tool-buck:bin", providers = [RunInfo])),
    },
    impl = _source_fix_impl,
)

# The package's :fix, once: every cargo_* macro defines it (buck/cargo/defs.bzl)
# with rust, over the crate's Rust files, and a package with no crate calls it
# itself. starlark names its Starlark files; a glob stops at a package below,
# so each file is fixed by the package that holds it. Generated files, which a
# tool writes as it formats them, are left out. directory is where the package
# is in the repository, for a package in a cell of its own, which also names
# the platform its :fix is built for, as the root cell's PACKAGE does for it.
def package_fix(rust = False, starlark = ["BUCK", "PACKAGE", "**/*.bzl", "**/*.bxl"], exclude = [], directory = None):
    if native.rule_exists("fix"):
        return
    package = native.package_name() if directory == None else directory
    prefix = package + "/" if package else ""
    paths = native.glob(starlark + (["**/*.rs"] if rust else []), exclude = exclude)
    platform = {} if directory == None else {"default_target_platform": "root//buck/platforms:linux_x86_64"}
    source_fix(name = "fix", srcs = {prefix + path: path for path in paths}, **platform)

# The binaries of crates/be-tool-bootstrap's release, compared with the ones
# scripts/internal/pins names (crates/be-tool-buck's unpinned), so that nothing
# is downloaded while they match: the default output lists each that does not,
# and [publish] holds each such binary too, for ./scripts/ci to upload and pin.
def _bootstrap_unpinned_impl(ctx: AnalysisContext) -> list[Provider]:
    def unpinned(name, publish):
        out = ctx.actions.declare_output(name, dir = True)
        ctx.actions.run(
            cmd_args(
                ctx.attrs._tools[RunInfo],
                "unpinned",
                out.as_output(),
                ctx.attrs.dist[DefaultInfo].default_outputs[0],
                ctx.attrs.pins,
                ["--publish"] if publish else [],
            ),
            category = "bootstrap_unpinned",
            identifier = name,
        )
        return out

    return [
        DefaultInfo(
            default_output = unpinned("unpinned", False),
            sub_targets = {"publish": [DefaultInfo(default_output = unpinned("publish", True))]},
        )
    ]

bootstrap_unpinned = rule(
    attrs = {
        "dist": attrs.dep(),
        "pins": attrs.source(),
        "_tools": attrs.default_only(attrs.exec_dep(default = "root//crates/be-tool-buck:bin", providers = [RunInfo])),
    },
    impl = _bootstrap_unpinned_impl,
)
