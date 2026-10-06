load("@prelude//test/inject_test_run_info.bzl", "inject_test_run_info")
load("//buck/cargo:defs.bzl", "test_run")

# Code that must not compile, and the errors it must fail with. Each case marks
# the line rustc should point at with `//~ ERROR <part of the message>`. The
# cases are one crate, compiled once by rustc's check pass: the test reads the
# crate's [diag.json], which the prelude writes whether or not rustc succeeds,
# so nothing is linked, and a case costs no more than the lines it adds. The
# crates live here rather than under crates/, so that building //crates/...,
# //:check and clippy never ask for the output rustc cannot make, and the
# autofixes that delete comments leave the markers alone.
def _compile_fail_test_impl(ctx: AnalysisContext) -> list[Provider]:
    diagnostics = ctx.attrs.cases[DefaultInfo].sub_targets["diag.json"][DefaultInfo].default_outputs[0]
    command = cmd_args(ctx.attrs._checker[RunInfo], "compile-fail", diagnostics, ctx.attrs.srcs)
    return inject_test_run_info(
        ctx,
        ExternalRunnerTestInfo(
            command = [command],
            labels = ctx.attrs.labels,
            run_from_project_root = True,
            type = "custom",
            use_project_relative_paths = True,
        ),
    ) + [DefaultInfo(default_output = diagnostics)]

_compile_fail_test = rule(
    attrs = {
        "cases": attrs.dep(),
        "labels": attrs.list(attrs.string(), default = []),
        "srcs": attrs.list(attrs.source()),
        "_checker": attrs.default_only(attrs.exec_dep(default = "//crates/buck-tools:buck-tools-bin", providers = [RunInfo])),
        "_inject_test_env": attrs.default_only(attrs.dep(default = "prelude//test/tools:inject_test_env")),
    },
    impl = _compile_fail_test_impl,
)

# The cases in <name>/, a crate whose root is <name>/lib.rs, checked against
# deps; the test is <name>-test, and <name>-test_run the action
# ./scripts/verify runs it as.
def compile_fail(name, deps):
    srcs = native.glob([name + "/**/*.rs"])
    native.rust_library(
        name = name,
        crate = name.replace("-", "_") + "_compile_fail",
        crate_root = name + "/lib.rs",
        deps = deps,
        edition = "2024",
        srcs = srcs,
    )
    _compile_fail_test(
        name = name + "-test",
        cases = ":" + name,
        srcs = srcs,
    )
    test_run(
        name = name + "-test_run",
        test = ":" + name + "-test",
    )
