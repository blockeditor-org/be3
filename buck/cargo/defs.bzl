load("@root//buck/dev:test.bzl", "be_test")
load("@root//buck/dev:verify.bzl", "package_fix")
load("@root//buck/platforms:cross.bzl", "PLAN_SETTINGS")
load("@root//buck/platforms:profile.bzl", "RELEASE_SYMBOL_FLAGS", "RUSTC_THREADS", "BINARY_LINK_STYLE", "RUST_LOCAL", "dev_only", "opt_flags")
load(":inputs.bzl", "cargo_inputs")
load(":layers.bzl", "check_layers")
load(":members.bzl", "members")

# The rules for a workspace crate, filled in from its Cargo.toml through the
# crate's facts: its PACKAGE file, which ./scripts/buckify generates from
# cargo's own plans and which sets them as the package value be3.cargo, so a
# change to one crate's dependencies re-evaluates that crate's package alone. A
# BUCK file passes what Cargo.toml cannot say as arguments: extra_deps and env
# are added to what the macro works out, and anything else goes to the rule as
# it is. Where a crate's dependencies or features differ between platforms, the
# macro selects them by plan, with the settings in PLAN_SETTINGS.

def _facts():
    crate = native.read_package_value("be3.cargo")
    if crate == None or crate["package"] != native.package_name():
        fail(
            "{} is not a workspace member cargo knows about: it has no PACKAGE of its own setting be3.cargo. Run ./scripts/buckify, which generates it from Cargo.toml.".format(
                native.package_name()
            )
        )
    return crate

# A crate package's own targets beside the ones its BUCK file asks for: :fix
# (buck/dev/verify.bzl) and :cargo-inputs (inputs.bzl, what buckify plans the
# crate from), after checking the crate's dependencies against the layers in
# layers.bzl. Every crate's BUCK file calls it once, before any other macro
# here or in buck/wasm/defs.bzl, unless it calls cargo_crate(), which calls it.
def cargo_package():
    check_layers(_facts())
    package_fix(exclude = ["PACKAGE"], rust = True)
    cargo_inputs()

# The crate's facts, for a macro that defines one of its targets.
def _crate():
    crate = _facts()
    if not native.rule_exists("cargo-inputs"):
        fail(
            "{}'s BUCK file defines a target before calling cargo_package(), which every crate's BUCK file calls first unless it calls cargo_crate()".format(
                native.package_name()
            )
        )
    return crate

# One value per platform, as a select() when they differ and a plain list when
# they do not. A platform cargo never planned the crate for gets nothing, since
# _compatible makes the crate's targets incompatible with it.
def _per_platform(crate, pick, extra = []):
    values = {platform: sorted(pick(entry) + extra) for platform, entry in crate["platforms"].items()}
    if not values:
        return sorted(extra)
    distinct = []
    for value in values.values():
        if value not in distinct:
            distinct.append(value)
    if len(distinct) == 1:
        return distinct[0]
    return select({"DEFAULT": []} | {setting: values.get(plan, []) for plan, setting in PLAN_SETTINGS.items()})

# What makes a crate's targets incompatible with every platform cargo did not
# plan it for (PLANS in crates/be-tool-buck/src/plan.rs says which), so that one
# asked for there, such as an app crate built for anything but the guest's
# wasi, fails when the build is configured, rather than building with no
# dependencies. It names a constraint no platform has, which buck2's error
# names. The settings are the ones the root PACKAGE picks a plan by. A binary
# or an example is compatible only with the plans that build it, which may be
# fewer than the crate's (`cargo build --lib` builds none), as `plans` names
# them.
def _planned(crate, plans = None):
    if plans == None:
        plans = crate["platforms"].keys()
    unplanned = [plan for plan in PLAN_SETTINGS if plan not in plans]
    if not unplanned:
        return []
    return select(
        {"DEFAULT": []} | {PLAN_SETTINGS[plan]: ["root//buck/constraints:planned_by_cargo"] for plan in unplanned} | {PLAN_SETTINGS[plan]: [] for plan in plans}
    )

def _compatible(crate, kwargs, plans = None):
    planned = _planned(crate, plans)
    if planned:
        kwargs["target_compatible_with"] = kwargs.get("target_compatible_with", []) + planned

def _env(crate, crate_name, env):
    base = {
        "CARGO_CRATE_NAME": crate_name,
        "CARGO_MANIFEST_DIR": native.package_name(),
        "CARGO_PKG_NAME": crate["name"],
        "CARGO_PKG_VERSION": crate["version"],
    }
    base.update(env)
    return base

# Files a build script or proc macro reads at CARGO_MANIFEST_DIR: the crate's
# own resources, and foreign ones, a target mapped to its path from the root,
# such as another crate's WIT files that this crate's WIT uses.
def cargo_manifest_sources(resources = [], foreign_resources = {}):
    sources = {"Cargo.toml": native.package_name() + "/Cargo.toml"}
    for resource in resources:
        sources[resource] = native.package_name() + "/" + resource
    sources.update(foreign_resources)
    return sources

# A library's sources, non-Rust files read at compile time included. Tests take
# all of src/** but src/bin/**, so an asset under a tests/ directory reaches the
# tests and not the library. Sources sit at their path in the package, not under
# CARGO_MANIFEST_DIR, so a file a proc macro reads from there is passed as
# `resources` (be-sdk-type-api's WIT is).
def cargo_library_sources():
    return native.glob(
        ["src/**"],
        exclude = [
            "src/bin/**",
            "src/**/tests.rs",
            "src/**/tests/**",
            "src/**/main_tests.rs",
            "src/**/main_tests/**",
        ],
    )

def cargo_test_sources():
    return native.glob(["src/**"], exclude = ["src/bin/**"])

# A binary or example in a crate with a library takes only its root file, or the
# root's directory when it is a main.rs below src/ or examples/.
def _target_sources(crate, root):
    if crate["library"] == None:
        return cargo_library_sources()
    if root.endswith("/main.rs") and root != "src/main.rs":
        return native.glob([root.removesuffix("main.rs") + "**"])
    return [root]

# The cfg that picks beui's HTML core, which the plans for its platforms
# (buck/constraints:html) were made under.
def _core_flags():
    return select({
        "DEFAULT": [],
        "root//buck/constraints:html": ["--cfg", "beui_html"],
    })

# Cargo.toml's [profile.dev.package] settings for the crate and the core's cfg,
# ahead of the flags its BUCK file passes, at a workspace crate's opt level
# (OPT_LEVELS in buck/platforms/profile.bzl). rustc's frontend runs on several
# threads (RUSTC_THREADS): the crates under beui are a chain, each typechecked
# only once the one below it is, and that chain is most of what a change to one
# of them waits on.
def _rustc_flags(crate, kwargs):
    if "srcs" in kwargs:
        fail("Rust sources are inferred; srcs is not supported")
    return (
        [RUSTC_THREADS] + RELEASE_SYMBOL_FLAGS + opt_flags("member") + dev_only(crate.get("profile_flags", [])) + _core_flags() + kwargs.pop("rustc_flags", [])
    )

# A proc macro's own visibility, which the root PACKAGE's default cannot give:
# the prelude's rust_library makes it an alias of the library, and gives the
# alias the visibility it was passed or none at all, where the library takes
# the package default.
def proc_macro_visibility(proc_macro, kwargs):
    if proc_macro and "visibility" not in kwargs:
        return {"visibility": ["PUBLIC"]}
    return {}

# The crate's library, named after the package. Another build of it, such as
# one for a fuzz target, takes a name and extra_features.
def cargo_library(name = None, extra_deps = [], extra_features = [], extra_srcs = [], resources = [], foreign_resources = {}, env = {}, **kwargs):
    crate = _crate()
    library = crate["library"]
    _compatible(crate, kwargs)
    native.rust_library(
        name = name or crate["name"],
        crate = library["crate"],
        crate_root = library["crate_root"],
        deps = _per_platform(crate, lambda entry: entry["deps"], extra_deps),
        edition = crate["edition"],
        env = _env(crate, library["crate"], env),
        features = _per_platform(crate, lambda entry: entry["features"], extra_features),
        mapped_srcs = cargo_manifest_sources(resources, foreign_resources),
        proc_macro = library["proc_macro"],
        rustc_flags = _rustc_flags(crate, kwargs),
        srcs = cargo_library_sources() + extra_srcs,
        **(kwargs | proc_macro_visibility(library["proc_macro"], kwargs) | RUST_LOCAL),
    )

# The library's own tests, which are #[cfg(test)] modules in its sources, with
# the dev-dependencies added, compiled as :test-harness and run as :test, a
# be_test (buck/dev/test.bzl), with env at compile time and run time and run_env
# at run time alone. A proc macro's tests need proc_macro named, since rustc
# takes no --extern for it by itself. LD_LIBRARY_PATH is the test's at run time
# alone: a sysroot's libc in it would load into the compiler.
#
# A test that paints (guides/testing_a_gui.md) reads and accepts the paintings
# in the crate's snapshots/, as app tests do (buck/wasm/defs.bzl).
# crates/be-host-tests is one: be-host with its apps, driven headless. With
# run_apart, each file under src/tests/ is a :test-<file> of its own, so a test
# that fails costs only its own paintings rather than every one the crate
# accepts, since a failed action keeps no outputs, and :test runs them all;
# apart_env adds to run_env for the files it names, as crates/be-host-tests
# names each test's own app.
def cargo_test(name = "test", extra_deps = [], env = {}, paints = False, run_apart = False, run_env = {}, apart_env = {}, resources = [], foreign_resources = {}, **kwargs):
    crate = _crate()
    library = crate["library"]
    _compatible(crate, kwargs)
    rustc_flags = _rustc_flags(crate, kwargs)
    if library["proc_macro"]:
        rustc_flags = rustc_flags + ["--extern", "proc_macro"]
    compile_env = _env(crate, library["crate"], {key: value for key, value in env.items() if key != "LD_LIBRARY_PATH"})
    native.rust_binary(
        name = name + "-harness",
        crate = library["crate"],
        crate_root = library["crate_root"],
        deps = _per_platform(crate, lambda entry: entry["deps"] + entry["test_deps"], extra_deps),
        edition = crate["edition"],
        env = compile_env,
        features = _per_platform(crate, lambda entry: entry["test_features"]),
        mapped_srcs = cargo_manifest_sources(resources, foreign_resources),
        rustc_flags = rustc_flags + ["--test"],
        srcs = cargo_test_sources(),
        **(BINARY_LINK_STYLE | kwargs | RUST_LOCAL),
    )
    test = dict(
        harness = ":" + name + "-harness",
        test_type = "rust",
        **{key: kwargs[key] for key in ["default_target_platform", "target_compatible_with", "visibility"] if key in kwargs},
    )
    if paints:
        test.update(manifest = "Cargo.toml", paintings = native.glob(["snapshots/*.paint"]), paints = True)
    run_env = compile_env | env | run_env
    parts = {}
    if run_apart:
        tests = [path.removeprefix("src/tests/").removesuffix(".rs") for path in native.glob(["src/tests/*.rs"])]
        for part in apart_env:
            if part not in tests:
                fail("apart_env names {}, which is not a file under src/tests/".format(part))
        for part in tests:
            be_test(
                name = "{}-{}".format(name, part),
                args = ["--exact", "tests::{}::{}".format(part, part)],
                env = run_env | apart_env.get(part, {}),
                part_of = ":" + name,
                **test,
            )
            parts[part] = ":{}-{}".format(name, part)
    be_test(name = name, env = run_env, parts = parts, **test)

# One of the crate's [[bin]] targets: :bin, or, in a crate with several, the
# binary's own name.
def cargo_binary(bin = None, name = None, extra_deps = [], env = {}, **kwargs):
    crate = _crate()
    binaries = {binary["name"]: binary for binary in crate["binaries"]}
    if bin == None:
        if len(binaries) != 1:
            fail("{} has {} binaries; name the one this is with bin".format(crate["name"], len(binaries)))
        bin = binaries.keys()[0]
    _compatible(crate, kwargs, [plan for plan, entry in crate["platforms"].items() if bin in entry["binaries"]])
    binary = binaries[bin]
    default = "bin" if len(binaries) == 1 else bin
    crate_name = bin.replace("-", "_")
    own = []
    if crate["library"] != None:
        own = [":" + crate["name"]]
    native.rust_binary(
        name = name or default,
        crate = crate_name,
        crate_root = binary["crate_root"],
        deps = _per_platform(crate, lambda entry: entry["binaries"].get(bin, []), own + extra_deps),
        edition = crate["edition"],
        env = _env(crate, crate_name, env),
        mapped_srcs = cargo_manifest_sources(),
        rustc_flags = _rustc_flags(crate, kwargs),
        srcs = _target_sources(crate, binary["crate_root"]),
        **(BINARY_LINK_STYLE | kwargs | RUST_LOCAL),
    )

# One of the crate's examples, as a binary, named example-<example>:
# `./scripts/buck run //crates/beui-host:example-survey`. It is built the way cargo
# builds it, with the library and the dev-dependencies.
def cargo_example(example, extra_deps = [], env = {}, **kwargs):
    crate = _crate()
    examples = {entry["name"]: entry for entry in crate["examples"]}
    if example not in examples:
        fail("{} has no example {}; run ./scripts/buckify, which regenerates the crate's PACKAGE".format(crate["name"], example))
    _compatible(crate, kwargs, [plan for plan, entry in crate["platforms"].items() if example in entry["examples"]])
    crate_name = example.replace("-", "_")
    own = []
    if crate["library"] != None:
        own = [":" + crate["name"]]
    native.rust_binary(
        name = "example-" + example,
        crate = crate_name,
        crate_root = examples[example]["crate_root"],
        deps = _per_platform(crate, lambda entry: entry["examples"].get(example, {}).get("deps", []), own + extra_deps),
        edition = crate["edition"],
        env = _env(crate, crate_name, env),
        features = _per_platform(crate, lambda entry: entry["examples"].get(example, {}).get("features", [])),
        mapped_srcs = cargo_manifest_sources(),
        rustc_flags = _rustc_flags(crate, kwargs),
        srcs = _target_sources(crate, examples[example]["crate_root"]),
        **(BINARY_LINK_STYLE | kwargs | RUST_LOCAL),
    )

# A crate's every target from its Cargo.toml: the library, if it has one, and
# its tests, unless test = False, every [[bin]] and every example, and
# cargo_package()'s. Most BUCK files are this one line, and a new crate starts
# with it; one that needs arguments for a target calls cargo_package() and then
# the macros above for each target instead.
def cargo_crate(test = True):
    cargo_package()
    crate = _crate()
    if crate["library"] != None:
        cargo_library()
        if test:
            cargo_test()
    for binary in crate["binaries"]:
        cargo_binary(bin = binary["name"])
    for example in crate["examples"]:
        cargo_example(example["name"])

# What the wasm macros in buck/wasm/defs.bzl need from a crate's Cargo.toml:
# the crate and its wasm dependencies, without the dev-dependencies and with,
# and what its targets are compatible with beyond wasm.
def cargo_wasm_facts():
    crate = _crate()
    library = crate["library"]
    return struct(
        name = crate["name"],
        compatible = _planned(crate),
        crate = library["crate"],
        crate_root = library["crate_root"],
        deps = _per_platform(crate, lambda entry: entry["deps"]),
        edition = crate["edition"],
        env = _env(crate, library["crate"], {}),
        features = _per_platform(crate, lambda entry: entry["features"]),
        test_deps = _per_platform(crate, lambda entry: entry["deps"] + entry["test_deps"]),
        test_features = _per_platform(crate, lambda entry: entry["test_features"]),
    )

# Every app's package, which is every workspace crate named be-app-*: what
# the host stages as its apps.
def app_packages():
    return sorted([package for package in members if package.startswith("crates/be-app-")])

# Every game's package, which is every workspace crate named tabletop-rules-*:
# what the games app stages as its data.
def game_packages():
    return sorted([package for package in members if package.startswith("crates/tabletop-rules-")])

# rustc's sancov pass, which libFuzzer reads coverage and comparisons from, and
# an optimised build that still checks for overflow and debug assertions, as
# cargo-fuzz builds one.
_FUZZ_FLAGS = (
    [
        "-Cpasses=sancov-module",
        "-Cllvm-args=-sanitizer-coverage-level=4",
        "-Cllvm-args=-sanitizer-coverage-inline-8bit-counters",
        "-Cllvm-args=-sanitizer-coverage-pc-table",
        "-Cllvm-args=-sanitizer-coverage-trace-compares",
    ]
    + opt_flags("fuzz")
    + [
        "-Cdebug-assertions=on",
        "-Coverflow-checks=on",
    ]
)

# Every file in a crate's fuzz/ is a coverage-guided fuzz target: a
# #![no_main] crate defining LLVMFuzzerTestOneInput over the crate's library,
# which is built again with extra_features and the coverage instrumentation, and
# linked with libFuzzer (//buck/tools:libfuzzer) for its main. fuzz/NAME.rs is
# :fuzz-NAME, which `./scripts/buck run` fuzzes until it is stopped, keeping
# what it finds in target/fuzz/<crate>/NAME (local-tools fuzz); //:fuzz runs
# every one, taking turns. :fuzz-NAME-test runs it briefly from a fixed seed, so a
# target that stops building or running fails verify.
def cargo_fuzz(extra_features = []):
    crate = _crate()
    library = "fuzz-lib"
    compatible = ["prelude//os:linux", "prelude//cpu:x86_64"]
    cargo_library(
        name = library,
        extra_features = extra_features,
        rustc_flags = _FUZZ_FLAGS,
        target_compatible_with = compatible,
        visibility = [],
    )
    for root in native.glob(["fuzz/*.rs"]):
        stem = root.removeprefix("fuzz/").removesuffix(".rs")
        name = "fuzz-" + stem
        native.rust_binary(
            name = name + "-bin",
            crate = stem.replace("-", "_"),
            crate_root = root,
            deps = [":" + library, "root//buck/tools:libfuzzer"],
            edition = crate["edition"],
            rustc_flags = _FUZZ_FLAGS,
            srcs = [root],
            target_compatible_with = compatible,
            **RUST_LOCAL,
        )
        fuzz_target(
            name = name,
            binary = ":{}-bin".format(name),
            id = crate["name"] + "/" + stem,
            target_compatible_with = compatible,
        )
        be_test(
            name = name + "-test",
            args = ["-runs=20000", "-seed=1"],
            harness = ":{}-bin".format(name),
            target_compatible_with = compatible,
        )

# What //buck/dev/fuzz.bxl finds every fuzz target by.
FuzzTargetInfo = provider(fields = {"binary": Artifact, "id": str})

# A fuzz target's binary under the id its corpus is kept by, run here by
# local-tools fuzz, which reads them from a list of "id<TAB>binary" lines:
# here the one, and for //:fuzz every one.
def _fuzz_target_impl(ctx: AnalysisContext) -> list[Provider]:
    binary = ctx.attrs.binary[DefaultInfo].default_outputs[0]
    targets = ctx.actions.write("targets", [cmd_args(ctx.attrs.id, binary, delimiter = "\t")])
    return [
        DefaultInfo(default_output = targets, other_outputs = [binary]),
        RunInfo(args = cmd_args(ctx.attrs._runner[RunInfo], "fuzz", "--list", targets, hidden = binary)),
        FuzzTargetInfo(binary = binary, id = ctx.attrs.id),
    ]

fuzz_target = rule(
    attrs = {
        "binary": attrs.dep(providers = [RunInfo]),
        "id": attrs.string(),
        "_runner": attrs.default_only(attrs.dep(default = "root//crates/be-tool-local:bin", providers = [RunInfo])),
    },
    impl = _fuzz_target_impl,
)
