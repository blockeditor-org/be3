load("@root//buck/dev:test.bzl", "THOROUGH", "be_test")

# A beui program started headless for an agent to drive with `drive`
# (guides/running/drive.md): `./scripts/buck run //crates/<crate>:dev`. Arguments
# after `--` go to `be-drive launch`, which passes on the ones it does not know.
# With archive, program is a path in that program archive (crates/be-host-archive), which
# be-drive unpacks, so only the archive is downloaded, and copies each of beside into
# (the program's source files, which its archive leaves out: buck/program/defs.bzl).
def headless(name, program, program_name, args = [], data = False, archive = None, beside = []):
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
        + ["--beside=" + file for file in beside]
        + (["--data"] if data else [])
        + args,
    )

# What a browser test needs on a worker, where it runs: Node, Playwright and the headless Chromium it installs, all
# pinned downloads (buck/tools), and the libraries that Chromium loads and the workers' image lacks
# (buck/sysroot:amd64-browser). Chromium runs as root with no sandbox, which browser.mjs asks for.
_BROWSER_ENV = {
    "BE3_CHROME": "$(location //buck/tools:chrome-headless-shell)/chrome-headless-shell",
    "LD_LIBRARY_PATH": "$(location //buck/sysroot:amd64-browser)/usr/lib/x86_64-linux-gnu",
    "NODE_PATH": "$(location //buck/tools:playwright-modules)",
}

# The drive scripts of a crate run in a browser against its web bundle on
# beui's HTML core (guides/testing_a_gui.md, 7): buck/program/web-test.mjs serves
# the bundle, opens each script's fixture in Chromium through Playwright and
# runs the script, and with checks also runs the page's own checks(). It runs
# on a worker, with the browser above, and fails, never skips, when it is not there.
# :<name> is the test (`./scripts/test //crates/<crate>:<name>`), and :<name>[run] the
# action ./scripts/verify builds.
def _web_drive_test_impl(ctx: AnalysisContext) -> list[Provider]:
    runner = ctx.attrs._scripts[DefaultInfo].default_outputs[0]
    command = cmd_args(
        cmd_args(ctx.attrs._node[DefaultInfo].default_outputs[0], format = "{}/bin/node"),
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
        "_node": attrs.default_only(attrs.dep(default = "root//buck/tools:node")),
        "_scripts": attrs.default_only(attrs.dep(default = "root//buck/program:drive-browser")),
    },
    impl = _web_drive_test_impl,
)

def web_drive_test(name, bundle, scripts, checks = False):
    _web_drive_test(name = name + "-harness", bundle = bundle, checks = checks, scripts = scripts)
    be_test(name = name, env = _BROWSER_ENV, harness = ":" + name + "-harness")

# A page that checks itself in Chromium: files are staged into one directory
# (the name each is served as, to its source, which may be a directory such as
# a bundle), and page-test.mjs serves it, opens it through Playwright and fails
# unless the page's globalThis.outcome says it passed. It runs on a worker, as
# web_drive_test does, with the same browser.
def _page_test_impl(ctx: AnalysisContext) -> list[Provider]:
    page = ctx.actions.copied_dir("page", ctx.attrs.files)
    runner = ctx.attrs._scripts[DefaultInfo].default_outputs[0]
    command = cmd_args(
        cmd_args(ctx.attrs._node[DefaultInfo].default_outputs[0], format = "{}/bin/node"),
        cmd_args(runner, format = "{}/page-test.mjs"),
        page,
    )
    return [DefaultInfo(default_output = page), RunInfo(args = command)]

_page_test = rule(
    attrs = {
        "files": attrs.dict(key = attrs.string(), value = attrs.source()),
        "_node": attrs.default_only(attrs.dep(default = "root//buck/tools:node")),
        "_scripts": attrs.default_only(attrs.dep(default = "root//buck/program:drive-browser")),
    },
    impl = _page_test_impl,
)

def page_test(name, files):
    _page_test(name = name + "-harness", files = files)
    be_test(name = name, env = _BROWSER_ENV, harness = ":" + name + "-harness")

# A test of the whole web host (crates/be-host's :web bundle with its apps) in Chromium:
# buck/program/web-host-test.mjs starts be-server and Caddy in front of the bundle, opens the
# page, signs in as web-dev does and runs `test`, a module whose default export takes the page
# and a drive function (the comment atop web-host-test.mjs). It runs on a worker, with the
# browser above.
def _web_host_test_impl(ctx: AnalysisContext) -> list[Provider]:
    runner = ctx.attrs._scripts[DefaultInfo].default_outputs[0]
    command = cmd_args(
        cmd_args(ctx.attrs._node[DefaultInfo].default_outputs[0], format = "{}/bin/node"),
        cmd_args(runner, format = "{}/web-host-test.mjs"),
        cmd_args(ctx.attrs.bundle[DefaultInfo].default_outputs[0], format = "--bundle={}"),
        cmd_args(ctx.attrs.server[RunInfo], format = "--server={}"),
        cmd_args(ctx.attrs._caddy[DefaultInfo].default_outputs[0], format = "--caddy={}"),
        cmd_args(ctx.attrs.caddyfile, format = "--caddyfile={}"),
        ctx.attrs.test,
    )
    return [DefaultInfo(), RunInfo(args = command)]

_web_host_test = rule(
    attrs = {
        "bundle": attrs.dep(),
        "caddyfile": attrs.source(),
        "server": attrs.dep(providers = [RunInfo]),
        "test": attrs.source(),
        "_caddy": attrs.default_only(attrs.dep(default = "root//buck/tools:caddy")),
        "_node": attrs.default_only(attrs.dep(default = "root//buck/tools:node")),
        "_scripts": attrs.default_only(attrs.dep(default = "root//buck/program:drive-browser")),
    },
    impl = _web_host_test_impl,
)

#
# Each is a thorough test (THOROUGH in buck/dev/test.bzl): one run is the whole web host with
# every app, drawn on SwiftShader, about 80 to 180 CPU seconds, rerun by any change to be-host or
# an app, so ./scripts/verify leaves it to ./scripts/verify --thorough.
def web_host_test(name, bundle, server, caddyfile, test):
    _web_host_test(name = name + "-harness", bundle = bundle, caddyfile = caddyfile, server = server, test = test)
    be_test(name = name, env = _BROWSER_ENV, harness = ":" + name + "-harness", labels = [THOROUGH])

# A program that drives Chromium itself, here rather than on a worker, through a Node script:
# be-tool-browser-rasterizer, which CI downloads to draw the browser paintings in the paint
# previews. Everything it runs is laid out in one directory, which is the default output, so it
# runs wherever that was downloaded with nothing installed:
#
#   DIR/<name>            program, which finds the rest beside its own path
#   DIR/<script's name>   each of scripts
#   DIR/node              the browser tests' Node, its executable alone
#   DIR/node_modules/     their Playwright, which a script beside it requires
#   DIR/chrome/chrome-headless-shell   their Chromium
#   DIR/lib/              the libraries it loads (buck/sysroot:amd64-browser), for LD_LIBRARY_PATH
#
# Each is copied, so the program's own path is in DIR. `./scripts/buck run :<name>` runs it. It is
# about 650 MB, two fifths of it Chromium and two fifths the libraries (Mesa's, which libgbm
# brings, are most of those).
def _browser_program_impl(ctx: AnalysisContext) -> list[Provider]:
    files = {
        ctx.attrs.name: ctx.attrs.program[DefaultInfo].default_outputs[0],
        "chrome": ctx.attrs._chrome[DefaultInfo].default_outputs[0],
        "lib": ctx.attrs._libraries[DefaultInfo].default_outputs[0].project("usr/lib/x86_64-linux-gnu"),
        "node": ctx.attrs._node[DefaultInfo].default_outputs[0].project("bin/node"),
        "node_modules": ctx.attrs._modules[DefaultInfo].default_outputs[0],
    }
    for script in ctx.attrs.scripts:
        files[script.basename] = script
    directory = ctx.actions.copied_dir(ctx.attrs.name, files)
    return [DefaultInfo(default_output = directory), RunInfo(args = cmd_args(directory, format = "{}/" + ctx.attrs.name))]

browser_program = rule(
    attrs = {
        "program": attrs.dep(),
        "scripts": attrs.list(attrs.source()),
        "_chrome": attrs.default_only(attrs.dep(default = "root//buck/tools:chrome-headless-shell")),
        "_libraries": attrs.default_only(attrs.dep(default = "root//buck/sysroot:amd64-browser")),
        "_modules": attrs.default_only(attrs.dep(default = "root//buck/tools:playwright-modules")),
        "_node": attrs.default_only(attrs.dep(default = "root//buck/tools:node")),
    },
    impl = _browser_program_impl,
)
