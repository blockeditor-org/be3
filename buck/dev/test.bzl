load("@prelude//test/inject_test_run_info.bzl", "inject_test_run_info")
load("@root//buck/platforms:profile.bzl", "LOCAL_BUILD")
load("@root//buck/tools:defs.bzl", "worker_properties")

# How long a test may run before crates/be-tool-run-test stops it. A test that
# hangs would otherwise hold its worker until the build server gives up on it,
# and every later run of the same action would wait on that one.
TEST_TIME_LIMIT = "20m"

# What a test runs, for ./scripts/verify and ./scripts/test (buck/dev/verify.bxl,
# buck/dev/test.bxl), which run it as an action rather than through buck2 test:
# `launcher` is crates/be-tool-run-test with the test's own flags, to which a
# runner adds its output flags, then "--" and `command`. `hidden` is what the
# test reads that the command does not name, such as the accepted paintings.
# A test that `paints` accepts paintings under UPDATE_SNAPSHOTS. `parts` is a
# run_apart test's tests, one be_test a file under src/tests/, by the file's
# name; such a test has no [run] of its own.
TestRunInfo = provider(
    fields = {
        "command": provider_field(typing.Any),
        "env": provider_field(dict),
        "hidden": provider_field(list),
        "launcher": provider_field(typing.Any),
        "local": provider_field(bool),
        "paints": provider_field(bool),
        "parts": provider_field(dict),
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

# The test as the action [run] is, which is how ./scripts/verify runs it: an
# action whose inputs have not changed comes from the cache, where buck2 test
# runs a test again unless a worker ran it. It fails, with the test's output,
# when the test does, and is not cached then. A test that does not paint
# outputs `passed`. One that paints accepts every painting, writing the ones
# that changed or are new to paintings/changed/ and, for each accepted painting
# it compared, the SHA-256 of what it read to paintings/used/ (that of nothing
# when there was none), so that the paintings themselves are never downloaded;
# ./scripts/verify copies into the crate's snapshots/ the paintings in changed/
# whose accepted painting still has that hash, or under --check fails on them.
def _run(ctx: AnalysisContext, info: TestRunInfo) -> Artifact:
    if info.paints:
        out = ctx.actions.declare_output("paintings", dir = True, has_content_based_path = False)
        flag = "--paintings"
        env = info.env | {"UPDATE_SNAPSHOTS": "1"}
    else:
        out = ctx.actions.declare_output("passed")
        flag = "--passed"
        env = info.env
    ctx.actions.run(
        cmd_args(info.launcher, flag, out.as_output(), "--", info.command, hidden = info.hidden),
        category = "painting_test" if info.paints else "test_run",
        env = env,
        local_only = info.local,
    )
    return out

# Every test: harness, a RunInfo, run with args and env through
# crates/be-tool-run-test, for buck2 test and as the action [run] alike. An
# LD_LIBRARY_PATH in env is the test's alone (the launcher's --library-path),
# since Namespace's workers replace an action's with their own. manifest is the
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
    )
    info = TestRunInfo(
        command = cmd_args(ctx.attrs.harness[RunInfo], ctx.attrs.args),
        env = env,
        hidden = ctx.attrs.paintings,
        launcher = launcher,
        local = ctx.attrs.local,
        paints = ctx.attrs.paints,
        parts = {name: part[TestRunInfo] for name, part in ctx.attrs.parts.items()},
    )
    command = cmd_args(launcher, "--", info.command, hidden = info.hidden)
    sub_targets = {}
    if not ctx.attrs.parts:
        sub_targets["run"] = [DefaultInfo(default_output = _run(ctx, info))]
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

be_test = rule(
    attrs = {
        "args": attrs.list(attrs.arg(), default = []),
        "env": attrs.dict(default = {}, key = attrs.string(), value = attrs.arg()),
        "harness": attrs.dep(providers = [RunInfo]),
        "labels": attrs.list(attrs.string(), default = []),
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
