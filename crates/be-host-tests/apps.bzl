load("@root//buck/cargo:defs.bzl", "app_packages")
load("@root//buck/program:defs.bzl", "program")

# The apps every test's host stages whatever else it names: the apps the host
# needs to start, open the dev workspace and show the Files.
BASE = ["file-tree", "workspace"]

# What a test's host runs from (be-lib-build-id's BE_BUNDLE): the apps it stages,
# which are all that is in the directory, and the libraries in a directory every
# test shares, beside the one be-host built. A host that held its own copy of the
# binary and of every library took 800 MB, and there are twenty.
def _host_with(name, apps, libraries = "exclude"):
    packages = ["//crates/be-app-" + app for app in apps]
    program(
        name = name,
        app_types = [package + ":types" for package in packages],
        compiled_only = True,
        data = [package + ":data" for package in packages],
        files = ["build-info.json"],
        index = libraries != "only",
        libraries = libraries,
        manifests = [package + ":manifest" for package in packages],
        modules = [package + ":module" for package in packages],
        precompiled = "//crates/be-host:precompiled",
    )

# Defines the apps each test's host stages, one :host-with-<apps> for each
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
            "BE_BUNDLE": "$(location :{}):$(location :host-libraries)".format(name),
            "HOST_TEST_APPS": ",".join(hosts[name]),
            "HOST_TEST_MANIFESTS": ":".join(["$(location //crates/be-app-{}:manifest)".format(app) for app in every]),
        }
    for name, staged in hosts.items():
        _host_with(name, staged, "include" if name == "host-with-all" else "exclude")
    _host_with("host-libraries", every, "only")
    return apart_env
