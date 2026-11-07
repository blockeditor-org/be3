load("//buck/dev:test.bzl", "be_test")

# Code that must not compile, and the errors it must fail with. Each case marks
# the line rustc should point at with `//~ ERROR <part of the message>`. The
# cases are one crate, compiled once by rustc's check pass: the test reads the
# crate's [diag.json], which the prelude writes whether or not rustc succeeds,
# so nothing is linked, and a case costs no more than the lines it adds. The
# crates live here rather than under crates/, so that building //crates/...
# and ./scripts/verify never ask for the output rustc cannot make.
def _compile_fail_check_impl(ctx: AnalysisContext) -> list[Provider]:
    diagnostics = ctx.attrs.cases[DefaultInfo].sub_targets["diag.json"][DefaultInfo].default_outputs[0]
    command = cmd_args(ctx.attrs._checker[RunInfo], "compile-fail", diagnostics, ctx.attrs.srcs)
    return [DefaultInfo(default_output = diagnostics), RunInfo(args = command)]

_compile_fail_check = rule(
    attrs = {
        "cases": attrs.dep(),
        "srcs": attrs.list(attrs.source()),
        "_checker": attrs.default_only(attrs.exec_dep(default = "//crates/be-tool-buck:bin", providers = [RunInfo])),
    },
    impl = _compile_fail_check_impl,
)

# The cases in <name>/, a crate whose root is <name>/lib.rs, checked against
# deps; the test is <name>-test, a be_test, and <name>-test[run] the action
# ./scripts/verify runs it as.
def compile_fail(name, deps):
    srcs = native.glob([name + "/**/*.rs"])
    native.rust_library(
        name = name,
        crate = name.replace("-", "_") + "_compile_fail",
        crate_root = name + "/lib.rs",
        deps = deps,
        edition = "2024",
        env = {"CARGO_MANIFEST_DIR": native.package_name() + "/" + name},
        mapped_srcs = {name + "/Cargo.toml": native.package_name() + "/" + name + "/Cargo.toml"},
        srcs = srcs,
    )
    _compile_fail_check(
        name = name + "-check",
        cases = ":" + name,
        srcs = srcs,
    )
    be_test(
        name = name + "-test",
        harness = ":" + name + "-check",
    )
