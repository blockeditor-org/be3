load("@root//buck/app:defs.bzl", "app")
load("@root//buck/cargo:defs.bzl", "editor_packages")

# What every test's app stages whatever its editors: the editors the app needs
# to start, open the dev workspace and show the Files.
BASE = ["file-tree", "workspace-ui"]

def _app(name, editors):
    packages = ["//crates/editors/" + editor for editor in editors]
    app(
        name = name,
        binary = "//crates/block-app:bin",
        compiled_only = True,
        data = [package + ":data" for package in packages],
        executable_name = "block-app",
        files = ["build-info.json"],
        index = True,
        manifests = [package + ":manifest" for package in packages],
        modules = [package + ":module" for package in packages],
        plugin_types = [package + ":types" for package in packages],
        precompiled = "//crates/block-app:precompiled",
    )

# Defines the app each test runs against, one :app-<editors> for each distinct
# set of editors and :app-all with every editor, and gives each test named in
# `editors` its app as apart_env for cargo_test, with the editors it stages and
# every editor's manifest, which AppTest names the missing editor from when the
# app shows a block no staged editor edits.
def app_test_apps(editors):
    every = [package.removeprefix("crates/editors/") for package in editor_packages()]
    apps = {"app-all": every}
    apart_env = {}
    for test, wanted in editors.items():
        for editor in wanted:
            if editor not in every:
                fail("{} names {}, which is not an editor under crates/editors".format(test, editor))
        extra = sorted({editor: True for editor in wanted if editor not in BASE}.keys())
        name = "app-" + ("+".join(extra) if extra else "base")
        apps[name] = sorted(BASE + extra)
        apart_env[test] = {
            "APP_TEST_EDITORS": ",".join(apps[name]),
            "APP_TEST_MANIFESTS": ":".join(["$(location //crates/editors/{}:manifest)".format(editor) for editor in every]),
            "BLOCK_APP": "$(location :{})/block-app".format(name),
        }
    for name, staged in apps.items():
        _app(name, staged)
    return apart_env
