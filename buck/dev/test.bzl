load("@prelude//test/inject_test_run_info.bzl", "inject_test_run_info")
load("@root//buck/platforms:profile.bzl", "LOCAL_BUILD")
load("@root//buck/tools:defs.bzl", "worker_properties")

# How long a test may run before crates/be-tool-run-test stops it. A test that
# hangs would otherwise hold its worker until the build server gives up on it,
# and every later run of the same action would wait on that one.
TEST_TIME_LIMIT = "20m"

# What a test runs, for ./scripts/verify and ./scripts/test --update
# (buck/dev/verify.bxl and buck/dev/test.bxl), which run it as an action
# (run_test) rather than through buck2 test: `launcher` is crates/be-tool-run-test with the test's
# own flags, to which a runner adds its output flags, then "--" and `command`.
# `hidden` is what the test reads that the command does not name, such as the
# accepted paintings. A test that `paints` accepts paintings under
# UPDATE_SNAPSHOTS. `parts` is a run_apart test's tests, one be_test a test
# file, by its part's name; such a test has no [run] of its own, and `runs` is
# each part's [run], where any other test's is its own. `libtest` is a part's
# test, the full name it runs with --exact, and empty for any other test.
TestRunInfo = provider(
    fields = {
        "command": provider_field(typing.Any),
        "env": provider_field(dict),
        "hidden": provider_field(list),
        "launcher": provider_field(typing.Any),
        "libtest": provider_field(str),
        "local": provider_field(bool),
        "paints": provider_field(bool),
        "parts": provider_field(dict),
        "runs": provider_field(list),
    },
)

# Where buck2 test runs a test: a worker, which can answer a test that passed
# from its cache, or this machine, for a local build and for a test that must
# run here.
def _executor(local: bool):
    if local or LOCAL_BUILD:
        return CommandExecutorConfig(local_enabled = True, remote_enabled = False)
    return CommandExecutorConfig(
        local_enabled = False,
        remote_cache_enabled = True,
        remote_enabled = True,
        remote_execution_properties = worker_properties(),
        remote_execution_use_case = "buck2-default",
    )

# The one action that runs a test, through crates/be-tool-run-test, for :test's
# [run] (what ./scripts/verify builds, buck/dev/verify.bxl) and for
# ./scripts/test --update (buck/dev/test.bxl) alike: an action whose inputs have
# not changed comes from the cache, where buck2 test runs a test again unless a
# worker ran it. `out` is what the launcher writes, by `mode`:
#
# - "passed": a file, written when the test passes. The action fails, with the
#   test's output, when the test does, and is not cached then.
# - "paintings": a directory, for a test that paints, which accepts every
#   painting (UPDATE_SNAPSHOTS), writing the ones that changed or are new to
#   paintings/changed/ and, for each accepted painting it compared, the SHA-256
#   of what it read to paintings/used/ (that of nothing when there was none),
#   so that the paintings themselves are never downloaded; ./scripts/verify
#   copies into the crate's snapshots/ the paintings in changed/ whose accepted
#   painting still has that hash, or under --check fails on them. It fails as
#   "passed" does.
# - "record": a directory holding status, the test's exit status, log, its
#   output, and changed/, the paintings it accepted when `update` is set. The
#   action succeeds when the test fails, so that its output is kept, and so a
#   failure is cached like a pass: env is where a caller puts what makes a run
#   asked for again a new action (test.bxl's --again).
#
# filters go to the test after the command, such as libtest's name filters.
def run_test(actions, info: TestRunInfo, out: Artifact, mode: str, filters: list = [], update: bool = False, env: dict = {}, identifier: str | None = None):
    flag = {"paintings": "--paintings", "passed": "--passed", "record": "--record"}[mode]
    env = info.env | env
    if mode == "paintings" or update:
        env["UPDATE_SNAPSHOTS"] = "1"
    actions.run(
        cmd_args(info.launcher, flag, out.as_output(), "--", info.command, filters, hidden = info.hidden),
        category = {"paintings": "painting_test", "passed": "test_run", "record": "test"}[mode],
        env = env,
        identifier = identifier,
        local_only = info.local,
    )

def _run(ctx: AnalysisContext, info: TestRunInfo) -> Artifact:
    if info.paints:
        out = ctx.actions.declare_output("paintings", dir = True, has_content_based_path = False)
    else:
        out = ctx.actions.declare_output("passed")
    run_test(ctx.actions, info, out, "paintings" if info.paints else "passed")
    return out

# Every test: harness, a RunInfo, run with args and env through
# crates/be-tool-run-test, for buck2 test and as the action [run] alike. An
# LD_LIBRARY_PATH in env is the test's alone (the launcher's --library-path),
# so a sysroot's libc in it does not load into the launcher. manifest is the
# crate's Cargo.toml, whose directory is the test's CARGO_MANIFEST_DIR, for a
# test that reads its snapshots/ from there. A local test, which needs this
# machine (a browser), runs here and has no time limit.
def _be_test_impl(ctx: AnalysisContext) -> list[Provider]:
    env = dict(ctx.attrs.env)
    library_path = env.pop("LD_LIBRARY_PATH", None)
    if ctx.attrs.manifest:
        env["CARGO_MANIFEST_DIR"] = cmd_args(ctx.attrs.manifest, parent = 1)
    launcher = cmd_args(
        ctx.attrs._launcher[RunInfo],
        ["--library-path", library_path] if library_path != None else [],
        [] if ctx.attrs.local else ["--time-limit", TEST_TIME_LIMIT],
        ["--one-test"] if ctx.attrs.libtest else [],
    )
    fields = dict(
        command = cmd_args(ctx.attrs.harness[RunInfo], ctx.attrs.args),
        env = env,
        hidden = ctx.attrs.paintings,
        launcher = launcher,
        libtest = ctx.attrs.libtest,
        local = ctx.attrs.local,
        paints = ctx.attrs.paints,
        parts = {name: part[TestRunInfo] for name, part in ctx.attrs.parts.items()},
    )
    sub_targets = {}
    if ctx.attrs.parts:
        runs = [part[DefaultInfo].sub_targets["run"][DefaultInfo].default_outputs[0] for part in ctx.attrs.parts.values()]
    else:
        run = _run(ctx, TestRunInfo(runs = [], **fields))
        sub_targets["run"] = [DefaultInfo(default_output = run)]
        runs = [run]
    info = TestRunInfo(runs = runs, **fields)
    command = cmd_args(launcher, "--", info.command, hidden = info.hidden)
    remote = not (ctx.attrs.local or LOCAL_BUILD)
    return [
        info,
        DefaultInfo(default_outputs = ctx.attrs.harness[DefaultInfo].default_outputs, sub_targets = sub_targets),
    ] + inject_test_run_info(
        ctx,
        ExternalRunnerTestInfo(
            command = [command],
            default_executor = _executor(ctx.attrs.local),
            env = env,
            labels = ctx.attrs.labels + (["local"] if ctx.attrs.local else []),
            run_from_project_root = True,
            supports_test_execution_caching = remote,
            type = ctx.attrs.test_type,
            use_project_relative_paths = True,
        ),
    )

_be_test = rule(
    attrs = {
        "args": attrs.list(attrs.arg(), default = []),
        "env": attrs.dict(default = {}, key = attrs.string(), value = attrs.arg()),
        "harness": attrs.dep(providers = [RunInfo]),
        "labels": attrs.list(attrs.string(), default = []),
        "libtest": attrs.string(default = ""),
        "local": attrs.bool(default = False),
        "manifest": attrs.option(attrs.source(), default = None),
        "paintings": attrs.list(attrs.source(), default = []),
        "paints": attrs.bool(default = False),
        "parts": attrs.dict(default = {}, key = attrs.string(), value = attrs.dep(providers = [TestRunInfo])),
        "test_type": attrs.string(default = "custom"),
        "_inject_test_env": attrs.default_only(attrs.dep(default = "prelude//test/tools:inject_test_env")),
        "_launcher": attrs.default_only(attrs.exec_dep(default = "root//crates/be-tool-run-test:bin", providers = [RunInfo])),
    },
    impl = _be_test_impl,
)

# A test, which ./scripts/verify runs ([tests] or [app-tests],
# buck/dev/verify.bxl) wherever it is, on its default_target_platform if it
# names one: a crate's :test, a part of one (part_of, its :test), or any other.
def be_test(name, part_of = None, **kwargs):
    _be_test(name = name, **kwargs)
