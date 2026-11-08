load("@root//buck/cargo:crates.bzl", "crates")
load("@root//buck/platforms:profile.bzl", "PROFILE_REFS", "keep_profile")
load(":targets.bzl", "BINARY_NAMES", "MODULES", "PACKAGES", "TESTS", "TEST_PLATFORMS", "UNTESTED")
load(":test.bzl", "TestRunInfo")

# //:verify, everything ./scripts/verify and ./scripts/ci build, and what it is
# made of besides the tests (buck/dev/test.bzl): each package's autofixes, the
# check pass and the bootstrap's pins. Each part is a target of its own, which
# can be built alone: `./scripts/buck build //crates/be-lib-geometry:fix`.

# The autofixes of one package, as crates/be-tool-buck's fix: srcs, by their
# paths in the repository, copied, fixed and compared with what they were.
# fix-rust-source and rustfmt fix the Rust files, starlark_fmt the BUCK,
# PACKAGE, .bzl and .bxl files.
def _source_fix_impl(ctx: AnalysisContext) -> list[Provider]:
    sources = ctx.actions.copied_dir("sources", ctx.attrs.srcs)
    out = ctx.actions.declare_output("fix", dir = True)
    rust = [path for path in ctx.attrs.srcs if path.endswith(".rs")]
    ctx.actions.run(
        cmd_args(
            ctx.attrs._tools[RunInfo],
            "fix",
            out.as_output(),
            sources,
            ["--fix-rust-source", ctx.attrs._fix_rust_source[RunInfo], "--rustfmt", ctx.attrs._rustfmt[DefaultInfo].default_outputs[0]] if rust else [],
            "--starlark-fmt",
            ctx.attrs._starlark_fmt[DefaultInfo].default_outputs[0],
            "--starlark-config",
            ctx.attrs._starlark_config,
        ),
        category = "fix",
        identifier = ctx.label.package or "root",
    )
    return [DefaultInfo(default_output = out)]

source_fix = rule(
    attrs = {
        "srcs": attrs.dict(key = attrs.string(), value = attrs.source()),
        "_fix_rust_source": attrs.default_only(attrs.exec_dep(default = "root//crates/be-tool-fix-rust-source:bin", providers = [RunInfo])),
        "_rustfmt": attrs.default_only(attrs.exec_dep(default = "root//buck/tools:rustfmt-sysroot")),
        "_starlark_config": attrs.default_only(attrs.source(default = "root//:starlark_fmt.json")),
        "_starlark_fmt": attrs.default_only(attrs.exec_dep(default = "root//buck/tools:starlark_fmt")),
        "_tools": attrs.default_only(attrs.exec_dep(default = "root//crates/be-tool-buck:bin", providers = [RunInfo])),
    },
    impl = _source_fix_impl,
)

# The package's :fix, once: every cargo_* macro defines it (buck/cargo/defs.bzl)
# with rust, over the crate's Rust files, and a package with no crate calls it
# itself. starlark names its Starlark files; a glob stops at a package below,
# so each file is fixed by the package that holds it. Generated files, which a
# tool writes as it formats them, are left out. directory is where the package
# is in the repository, for a package in a cell of its own, which also names
# the platform its :fix is built for, as the root cell's PACKAGE does for it.
def package_fix(rust = False, starlark = ["BUCK", "PACKAGE", "**/*.bzl", "**/*.bxl"], exclude = [], directory = None):
    if native.rule_exists("fix"):
        return
    label = "{}//{}:fix".format(native.get_cell_name(), native.package_name())
    if not rust and label not in PACKAGES:
        fail("{} is a :fix //:verify would not build: name it in PACKAGES in buck/dev/targets.bzl".format(label))
    package = native.package_name() if directory == None else directory
    prefix = package + "/" if package else ""
    paths = native.glob(starlark + (["**/*.rs"] if rust else []), exclude = exclude)
    platform = {} if directory == None else {"default_target_platform": "root//buck/platforms:linux_x86_64"}
    source_fix(name = "fix", srcs = {prefix + path: path for path in paths}, visibility = ["root//:"], **platform)

# The binaries of crates/be-tool-bootstrap's release, compared with the ones
# scripts/internal/pins names (crates/be-tool-buck's unpinned), so that nothing
# is downloaded while they match: the default output lists each that does not,
# and [publish] holds each such binary too, for ./scripts/ci to upload and pin.
def _bootstrap_unpinned_impl(ctx: AnalysisContext) -> list[Provider]:
    def unpinned(name, publish):
        out = ctx.actions.declare_output(name, dir = True)
        ctx.actions.run(
            cmd_args(
                ctx.attrs._tools[RunInfo],
                "unpinned",
                out.as_output(),
                ctx.attrs.dist[DefaultInfo].default_outputs[0],
                ctx.attrs.pins,
                ["--publish"] if publish else [],
            ),
            category = "bootstrap_unpinned",
            identifier = name,
        )
        return out

    return [
        DefaultInfo(
            default_output = unpinned("unpinned", False),
            sub_targets = {"publish": [DefaultInfo(default_output = unpinned("publish", True))]},
        )
    ]

bootstrap_unpinned = rule(
    attrs = {
        "dist": attrs.dep(),
        "pins": attrs.source(),
        "_tools": attrs.default_only(attrs.exec_dep(default = "root//crates/be-tool-buck:bin", providers = [RunInfo])),
    },
    impl = _bootstrap_unpinned_impl,
)

# The platforms //:verify builds everything for but the tools it runs here:
# linux_x86_64, and for a test TEST_PLATFORMS names, linux_x86_64_html. Each is
# the platform as a target's own default configures it, so a command that
# builds the target alone shares its outputs.
def _linux_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.linux_x86_64[PlatformInfo], refs)

_linux = transition(impl = _linux_impl, refs = {"linux_x86_64": "root//buck/platforms:linux_x86_64"} | PROFILE_REFS)

def _html_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.linux_x86_64_html[PlatformInfo], refs)

_html = transition(impl = _html_impl, refs = {"linux_x86_64_html": "root//buck/platforms:linux_x86_64_html"} | PROFILE_REFS)

def _first(dep) -> Artifact:
    return dep[DefaultInfo].default_outputs[0]

# What each of //:verify's parts reads afterwards, as (kind, artifact), and
# what it only builds, by subtarget; "" is the target itself, which every
# command builds.
def _parts(ctx: AnalysisContext) -> dict:
    plain, paintings = [], []
    for test in ctx.attrs.linux_tests + ctx.attrs.html_tests:
        info = test[TestRunInfo]
        for run in info.runs:
            if info.paints:
                paintings.append(("paintings\t" + test.label.package, run))
            else:
                plain.append(run)
    checks = [output for check in ctx.attrs.checks for output in check[DefaultInfo].default_outputs]
    return {
        "": ([("verify", _first(ctx.attrs.verify)), ("buckify", _first(ctx.attrs.buckify)), ("sysroot", _first(ctx.attrs.sysroot))], []),
        "app-tests": (paintings, []),
        "bootstrap": ([("bootstrap", _first(ctx.attrs.bootstrap))], []),
        "lint": ([("fix", _first(fix)) for fix in ctx.attrs.fixes], []),
        "previews": (
            [
                ("local-tools", _first(ctx.attrs.local_tools)),
                ("browser-rasterizer", _first(ctx.attrs.browser_rasterizer)),
                ("paint-review-site", _first(ctx.attrs.paint_review_site)),
            ],
            [],
        ),
        "tests": ([], plain + checks),
    }

# The build builds a part's artifacts; the manifest is the part's
# "kind<TAB>path" lines, or for a test's paintings
# "paintings<TAB>package<TAB>path", the package holding the snapshots/ they
# belong in, each path absolute. It is a target of its own because a target
# one of whose actions failed reports no outputs, and with --keep-going
# crates/be-tool-verify reads whatever did build: an action that failed can
# still leave an earlier run's output at its path, so it writes a fix or a
# painting only where the checkout still holds what it was made from.
def _verify_impl(ctx: AnalysisContext) -> list[Provider]:
    infos = {}
    for name, (read, built) in _parts(ctx).items():
        if ctx.attrs.manifest:
            lines = [cmd_args(kind, artifact, delimiter = "\t") for kind, artifact in read]
            manifest = ctx.actions.write("manifest-{}.txt".format(name or "verify"), lines, absolute = True)
            infos[name] = DefaultInfo(default_output = manifest)
        else:
            infos[name] = DefaultInfo(other_outputs = [artifact for _, artifact in read] + built)
    return [
        DefaultInfo(
            default_outputs = infos[""].default_outputs,
            other_outputs = infos[""].other_outputs,
            sub_targets = {name: [info] for name, info in infos.items() if name},
        )
    ]

_verify = rule(
    attrs = {
        "bootstrap": attrs.transition_dep(cfg = _linux, default = "root//scripts/internal:bootstrap-unpinned[publish]"),
        "browser_rasterizer": attrs.dep(default = "root//crates/be-tool-browser-rasterizer:release"),
        "buckify": attrs.transition_dep(cfg = _linux),
        "checks": attrs.list(attrs.transition_dep(cfg = _linux)),
        "fixes": attrs.list(attrs.transition_dep(cfg = _linux)),
        "html_tests": attrs.list(attrs.transition_dep(cfg = _html, providers = [TestRunInfo])),
        "linux_tests": attrs.list(attrs.transition_dep(cfg = _linux, providers = [TestRunInfo])),
        "local_tools": attrs.dep(default = "root//crates/be-tool-local:release"),
        "manifest": attrs.bool(),
        "paint_review_site": attrs.transition_dep(cfg = _linux, default = "root//crates/be-tool-paint-review-web:web-release"),
        "sysroot": attrs.transition_dep(cfg = _linux, default = "root//buck/sysroot:lock"),
        "verify": attrs.dep(default = "root//crates/be-tool-verify:release"),
    },
    impl = _verify_impl,
)

# The crates under //crates a crate's dependencies on platform name, in a plan
# of crates.bzl's for it.
def _dependencies(entry: dict, keys: list[str]) -> list[str]:
    found = []
    for key in keys:
        value = entry[key]
        for each in value.values() if type(value) == type({}) else [value]:
            found.extend([dep.removeprefix("//").partition(":")[0] for dep in each if dep.startswith("//crates/")])
    return found

# The check pass ([check], rustc's metadata build with every lint) over each
# first-party Rust target on linux_x86_64 that nothing else verify builds
# compiles: a warning fails a compile, so a target that is compiled anyway is
# already checked, and checking it again would be a second rustc run for most
# crates. That is every binary and example, every proc macro (which its
# dependents build for the execution platform instead), and every library that
# no test, binary, example or checked library built for linux_x86_64 depends
# on, as crates.bzl's plans say. Those name a crate's own dependencies, not
# the targets a BUCK file adds, so a library built only through those, such as
# be-host's by crates/be-host-tests, is checked too. A test that may build for
# another platform (a crate planned for wasi-guest, or one TEST_PLATFORMS
# names) is taken to build nothing here. On wasi-guest, a library is built by
# the app or type module that holds it, as MODULES' are on every platform.
def _checks() -> list[str]:
    platform = "linux-x86_64"
    planned = {path: crate for path, crate in crates.items() if platform in crate["platforms"]}
    libraries = [path for path, crate in planned.items() if crate["library"] != None and path not in MODULES]
    proc_macros = {path: True for path in libraries if planned[path]["library"]["proc_macro"]}
    libraries = {path: True for path in libraries}
    covered = {}

    # Marks the libraries these are, and everything they build on, as built.
    def cover(paths):
        stack = list(paths)
        for _ in range(len(planned) * len(planned) + 1):
            if not stack:
                break
            path = stack.pop()
            if path in covered or path in proc_macros or path not in libraries:
                continue
            covered[path] = True
            stack.extend(_dependencies(planned[path]["platforms"][platform], ["deps"]))

    checks = []
    for path, crate in planned.items():
        entry = crate["platforms"][platform]
        tested = path not in UNTESTED and "wasi-guest" not in crate["platforms"] and "root//{}:test".format(path) not in TEST_PLATFORMS
        if tested:
            cover(_dependencies(entry, ["deps", "test_deps"]))
        for binary in crate["binaries"]:
            default = "bin" if len(crate["binaries"]) == 1 else binary["name"]
            checks.append("{}:{}".format(path, BINARY_NAMES.get(path, {}).get(binary["name"], default)))
        for example in crate["examples"]:
            checks.append("{}:example-{}".format(path, example["name"]))
        if crate["binaries"] or crate["examples"]:
            cover(_dependencies(entry, ["binaries", "examples"]) + [path])

    # A library is checked once nothing that might still be checked depends
    # on it, so that its dependents' checks are counted first.
    dependents = {path: [] for path in libraries}
    for path in libraries:
        for dependency in _dependencies(planned[path]["platforms"][platform], ["deps"]):
            if dependency in dependents and path not in proc_macros:
                dependents[dependency].append(path)
    for path in proc_macros:
        checks.append("{}:_{}".format(path, planned[path]["name"]))
    for _ in range(len(libraries) + 1):
        ready = [
            path for path in libraries if path not in covered and path not in proc_macros and not [each for each in dependents[path] if each not in covered]
        ]
        if not ready:
            break
        for path in ready:
            checks.append("{}:{}".format(path, planned[path]["name"]))
        cover(ready)
    return sorted(["root//{}[check]".format(check) for check in checks])

# //:verify and //:verify-manifest (_verify_impl): the autofixes of every
# crate crates.bzl names and every package PACKAGES does, every crate's :test
# but UNTESTED's and every one TESTS names, the check pass (_checks), the
# generated crates.bzl (cargo_plan, buck/cargo/buckify.bzl) and the sysroot's
# lockfile, and crates/be-tool-verify, which reads the rest afterwards. Each
# subtarget is a part of it, as ./scripts/verify's flags pick them: [lint] the
# autofixes, [tests] the tests that do not paint and the check pass,
# [app-tests] those that paint, [previews] the paint preview renderer and the
# site, [bootstrap] crates/be-tool-bootstrap's binaries that
# scripts/internal/pins does not name (//scripts/internal:bootstrap-unpinned's
# [publish]). The tools run here, crates/be-tool-verify and those for the
# previews, are built for the platform the command names (--target-platforms),
# and the rest for linux_x86_64 (guides/build_server.md).
#
#   ./scripts/buck build --keep-going //:verify[tests] //:verify-manifest[tests]
def verify(name: str, buckify: str):
    tests = ["root//{}:test".format(path) for path in crates if path not in UNTESTED] + TESTS
    for label in TEST_PLATFORMS.values():
        if label != "root//buck/platforms:linux_x86_64_html":
            fail("//:verify builds no test for {}".format(label))
    attributes = dict(
        buckify = buckify,
        checks = _checks(),
        fixes = ["root//{}:fix".format(path) for path in crates] + PACKAGES,
        html_tests = [test for test in tests if test in TEST_PLATFORMS],
        linux_tests = [test for test in tests if test not in TEST_PLATFORMS],
    )
    _verify(name = name, manifest = False, **attributes)
    _verify(name = name + "-manifest", manifest = True, **attributes)
