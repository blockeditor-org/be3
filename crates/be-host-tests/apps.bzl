load("@root//buck/program:defs.bzl", "program")
load("@root//buck/cargo:defs.bzl", "app_packages")

# The apps every test's host stages whatever else it names: the apps the host
# needs to start, open the dev workspace and show the Files.
BASE = ["file-tree", "workspace"]

def _host_with(name, apps):
    packages = ["//crates/be-app-" + app for app in apps]
    program(
        name = name,
        binary = "//crates/be-host:bin",
        compiled_only = True,
        data = [package + ":data" for package in packages],
        executable_name = "block-app",
        files = ["build-info.json"],
        index = True,
        manifests = [package + ":manifest" for package in packages],
        modules = [package + ":module" for package in packages],
        app_types = [package + ":types" for package in packages],
        precompiled = "//crates/be-host:precompiled",
    )

# Defines the host each test runs against, one :host-with-<apps> for each
# distinct set of apps and :host-with-all with every app, and gives each test
# named in `apps` its host as apart_env for cargo_test, with the apps it stages
# and every app's manifest, which HostTest names the missing app from when the
# host shows a block no staged app edits.
def host_test_apps(apps):
    every = [package.removeprefix("crates/be-app-") for package in app_packages()]
    hosts = {"host-with-all": every}
    apart_env = {}
    tests = [path.removeprefix("src/tests/").removesuffix(".rs") for path in glob(["src/tests/*.rs"])]
    for test, wanted in apps.items():
        if test not in tests:
            fail("{} names no test under crates/be-host-tests/src/tests".format(test))
        for app in wanted:
            if app not in every:
                fail("{} names {}, which is not an app: there is no crates/be-app-{}".format(test, app, app))
        extra = sorted({app: True for app in wanted if app not in BASE}.keys())
        name = "host-with-" + ("+".join(extra) if extra else "base")
        hosts[name] = sorted(BASE + extra)
        apart_env[test] = {
            "APP_TEST_EDITORS": ",".join(hosts[name]),
            "APP_TEST_MANIFESTS": ":".join(["$(location //crates/be-app-{}:manifest)".format(app) for app in every]),
            "BLOCK_APP": "$(location :{})/block-app".format(name),
        }
    for name, staged in hosts.items():
        _host_with(name, staged)
    return apart_env
