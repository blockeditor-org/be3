load("@root//buck/dev:test.bzl", "be_test")

# A beui program started headless for an agent to drive with `drive`
# (guides/running/drive.md): `./scripts/buck run //crates/<crate>:dev`. Arguments
# after `--` go to `be-drive launch`, which passes on the ones it does not know.
# With archive, program is a path in that program archive (crates/be-host-archive), which
# be-drive unpacks, so only the archive is downloaded.
def headless(name, program, program_name, args = [], data = False, archive = None):
    native.command_alias(
        name = name,
        args = [
            "$(location //crates/be-tool-drive:bin)",
            "launch",
            "--name=" + program_name,
            "--libraries=$(location //buck/sysroot:amd64-libraries)",
            "--program=" + program,
        ]
        + (["--archive=" + archive] if archive else [])
        + (["--data"] if data else [])
        + args,
    )

# The drive scripts of a crate run in a browser against its web bundle on
# beui's HTML core (guides/testing_a_gui.md, 7): buck/program/web-test.mjs serves
# the bundle, opens each script's fixture in Chromium through Playwright and
# runs the script, and with checks also runs the page's own checks(). It runs
# on this machine, since the workers have no browser: BE3_CHROME when set,
# else Playwright's Chromium, and it fails, never skips, when neither is there.
# :<name> is the test (`./scripts/test //crates/<crate>:<name>`), a local
# be_test, and :<name>[run] the action ./scripts/verify builds.
def _web_drive_test_impl(ctx: AnalysisContext) -> list[Provider]:
    runner = ctx.attrs._scripts[DefaultInfo].default_outputs[0]
    command = cmd_args(
        "node",
        cmd_args(runner, format = "{}/web-test.mjs"),
        cmd_args(ctx.attrs.bundle[DefaultInfo].default_outputs[0], format = "--bundle={}"),
        ["--checks"] if ctx.attrs.checks else [],
        ctx.attrs.scripts,
    )
    return [DefaultInfo(), RunInfo(args = command)]

_web_drive_test = rule(
    attrs = {
        "bundle": attrs.dep(),
        "checks": attrs.bool(default = False),
        "scripts": attrs.list(attrs.source()),
        "_scripts": attrs.default_only(attrs.dep(default = "root//buck/program:drive-browser")),
    },
    impl = _web_drive_test_impl,
)

def web_drive_test(name, bundle, scripts, checks = False):
    _web_drive_test(name = name + "-harness", bundle = bundle, checks = checks, scripts = scripts)
    be_test(name = name, harness = ":" + name + "-harness", local = True)

# A page that checks itself in Chromium: files are staged into one directory
# (the name each is served as, to its source, which may be a directory such as
# a bundle), and page-test.mjs serves it, opens it through Playwright and fails
# unless the page's globalThis.outcome says it passed. It runs on this
# machine, as web_drive_test does, with BE3_CHROME or Playwright's Chromium.
def _page_test_impl(ctx: AnalysisContext) -> list[Provider]:
    page = ctx.actions.copied_dir("page", ctx.attrs.files)
    command = cmd_args("node", ctx.attrs.script, page)
    return [DefaultInfo(default_output = page), RunInfo(args = command)]

_page_test = rule(
    attrs = {
        "files": attrs.dict(key = attrs.string(), value = attrs.source()),
        "script": attrs.source(default = "root//buck/program:page-test.mjs"),
    },
    impl = _page_test_impl,
)

def page_test(name, files):
    _page_test(name = name + "-harness", files = files)
    be_test(name = name, harness = ":" + name + "-harness", local = True)
