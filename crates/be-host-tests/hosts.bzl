load("@root//buck/program:defs.bzl", "program")
load("@root//buck/cargo:defs.bzl", "editor_packages")

# What every test's host stages whatever its editors: the editors the host needs
# to start, open the dev workspace and show the Files.
BASE = ["file-tree", "workspace"]

def _host(name, editors):
    packages = ["//crates/be-app-" + editor for editor in editors]
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
        plugin_types = [package + ":types" for package in packages],
        precompiled = "//crates/be-host:precompiled",
    )

# Defines the host each test runs against, one :host-<editors> for each distinct
# set of editors and :host-all with every editor, and gives each test named in
# `editors` its host as apart_env for cargo_test, with the editors it stages and
# every editor's manifest, which HostTest names the missing editor from when the
# host shows a block no staged editor edits.
def host_test_hosts(editors):
    every = [package.removeprefix("crates/be-app-") for package in editor_packages()]
    hosts = {"host-all": every}
    apart_env = {}
    tests = [path.removeprefix("src/tests/").removesuffix(".rs") for path in glob(["src/tests/*.rs"])]
    for test, wanted in editors.items():
        if test not in tests:
            fail("{} names no test under crates/be-host-tests/src/tests".format(test))
        for editor in wanted:
            if editor not in every:
                fail("{} names {}, which is not an editor: there is no crates/be-app-{}".format(test, editor, editor))
        extra = sorted({editor: True for editor in wanted if editor not in BASE}.keys())
        name = "host-" + ("+".join(extra) if extra else "base")
        hosts[name] = sorted(BASE + extra)
        apart_env[test] = {
            "APP_TEST_EDITORS": ",".join(hosts[name]),
            "APP_TEST_MANIFESTS": ":".join(["$(location //crates/be-app-{}:manifest)".format(editor) for editor in every]),
            "BLOCK_APP": "$(location :{})/block-app".format(name),
        }
    for name, staged in hosts.items():
        _host(name, staged)
    return apart_env
