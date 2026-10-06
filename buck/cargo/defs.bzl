load("@prelude//rust:cargo_package.bzl", "apply_platform_attrs", "get_reindeer_platform_names")
load("@root//buck/platforms:profile.bzl", "dev_only")
load(":crates.bzl", "crates")

# The rules for a workspace crate, filled in from its Cargo.toml through
# crates.bzl, which ./scripts/buck generates from cargo's own plans. A BUCK file
# passes what Cargo.toml cannot say as arguments: extra_deps and env are added
# to what the macro works out, and anything else goes to the rule as it is.
# Where a crate's dependencies or features differ between platforms, the macro
# selects them by plan through the prelude's apply_platform_attrs, which the
# third-party rules use too, from the map the root PACKAGE sets.

def _crate():
    package = native.package_name()
    if package not in crates:
        fail("{} is not a workspace member cargo knows about; run ./scripts/buck, which regenerates buck/cargo/crates.bzl".format(package))
    return crates[package]

# One value per platform, as a select() when they differ and a plain list when
# they do not. A crate cargo never builds for a platform still needs a value
# there, and gets the first platform it is built for's.
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
    fallback = values[sorted(values)[0]]
    per_plan = {platform: {"value": values.get(platform, fallback)} for platform in get_reindeer_platform_names()}
    return apply_platform_attrs(per_plan, {})["value"]

def _env(crate, crate_name, env):
    base = {
        "CARGO_CRATE_NAME": crate_name,
        "CARGO_MANIFEST_DIR": native.package_name(),
        "CARGO_PKG_NAME": crate["name"],
        "CARGO_PKG_VERSION": crate["version"],
    }
    base.update(env)
    return base

def _srcs(kwargs):
    return kwargs.pop("srcs", native.glob(["src/**/*.rs"]))

# Cargo.toml's [profile.dev.package] settings for the crate, ahead of the flags
# its BUCK file passes.
def _rustc_flags(crate, kwargs):
    return dev_only(crate.get("profile_flags", [])) + kwargs.pop("rustc_flags", [])

# The crate's library, named after the package.
def cargo_library(name = None, extra_deps = [], env = {}, **kwargs):
    crate = _crate()
    library = crate["library"]
    native.rust_library(
        name = name or crate["name"],
        crate = library["crate"],
        crate_root = library["crate_root"],
        deps = _per_platform(crate, lambda entry: entry["deps"], extra_deps),
        edition = crate["edition"],
        env = _env(crate, library["crate"], env),
        features = _per_platform(crate, lambda entry: entry["features"]),
        proc_macro = library["proc_macro"],
        rustc_flags = _rustc_flags(crate, kwargs),
        srcs = _srcs(kwargs),
        visibility = kwargs.pop("visibility", ["PUBLIC"]),
        **kwargs,
    )

# The library's own tests, which are #[cfg(test)] modules in its sources, with
# the dev-dependencies added. A proc macro's tests need proc_macro named, since
# rust_test has no attribute for it.
#
# A test whose env names LD_LIBRARY_PATH is a library_path_test around the
# test harness, built as a binary, since Namespace's workers replace an
# action's LD_LIBRARY_PATH with their own.
def cargo_test(name = "test", extra_deps = [], env = {}, **kwargs):
    crate = _crate()
    library = crate["library"]
    rustc_flags = _rustc_flags(crate, kwargs)
    if library["proc_macro"]:
        rustc_flags = rustc_flags + ["--extern", "proc_macro"]
    common = dict(
        crate = library["crate"],
        crate_root = library["crate_root"],
        deps = _per_platform(crate, lambda entry: entry["deps"] + entry["test_deps"], extra_deps),
        edition = crate["edition"],
        features = _per_platform(crate, lambda entry: entry["test_features"]),
        srcs = _srcs(kwargs),
    )
    if "LD_LIBRARY_PATH" in env:
        compile_env = {key: value for key, value in env.items() if key != "LD_LIBRARY_PATH"}
        native.rust_binary(
            name = name + "-harness",
            env = _env(crate, library["crate"], compile_env),
            rustc_flags = rustc_flags + ["--test"],
            target_compatible_with = kwargs.get("target_compatible_with", []),
            **common,
        )
        library_path_test(name = name, env = _env(crate, library["crate"], env), harness = ":" + name + "-harness", **kwargs)
    else:
        native.rust_test(name = name, env = _env(crate, library["crate"], env), rustc_flags = rustc_flags, **(common | kwargs))
    if kwargs.get("remote_execution") != "disabled":
        test_run(name = name + "_run", test = ":" + name)

# A Rust test harness run with its LD_LIBRARY_PATH carried as
# BE3_LD_LIBRARY_PATH and put back by the shell that starts it, so it survives
# a worker that sets its own.
_restore_library_path = 'LD_LIBRARY_PATH="$BE3_LD_LIBRARY_PATH" && export LD_LIBRARY_PATH && exec "$@"'

def _library_path_test_impl(ctx: AnalysisContext) -> list[Provider]:
    env = {key: value for key, value in ctx.attrs.env.items() if key != "LD_LIBRARY_PATH"}
    env["BE3_LD_LIBRARY_PATH"] = ctx.attrs.env["LD_LIBRARY_PATH"]
    command = cmd_args("sh", "-c", _restore_library_path, "sh", ctx.attrs.harness[RunInfo])
    return [
        DefaultInfo(default_outputs = ctx.attrs.harness[DefaultInfo].default_outputs),
        RunInfo(args = command),
        ExternalRunnerTestInfo(
            command = [command],
            env = env,
            labels = ctx.attrs.labels,
            run_from_project_root = True,
            type = "rust",
            use_project_relative_paths = True,
        ),
    ]

library_path_test = rule(
    attrs = {
        "env": attrs.dict(key = attrs.string(), value = attrs.arg()),
        "harness": attrs.dep(providers = [RunInfo]),
        "labels": attrs.list(attrs.string(), default = []),
    },
    impl = _library_path_test_impl,
)

# A test as an action, which is how //:verify runs it: buck2 runs a test again
# every time, where an action whose binary and inputs have not changed comes
# from the cache. It runs the test's own command and environment on a worker,
# and fails, with the test's output, when the test does.
def _test_run_impl(ctx: AnalysisContext) -> list[Provider]:
    test = ctx.attrs.test[ExternalRunnerTestInfo]
    passed = ctx.actions.declare_output("passed")
    ctx.actions.run(
        cmd_args(
            "sh",
            "-c",
            'passed="$1" && shift && "$@" && : > "$passed"',
            "sh",
            passed.as_output(),
            test.command,
        ),
        category = "test_run",
        env = test.env,
    )
    return [DefaultInfo(default_output = passed)]

test_run = rule(
    attrs = {"test": attrs.dep(providers = [ExternalRunnerTestInfo])},
    impl = _test_run_impl,
)

# One of the crate's [[bin]] targets. A binary named like its package is
# <name>-bin, since the library already has the name.
def cargo_binary(bin = None, name = None, extra_deps = [], env = {}, **kwargs):
    crate = _crate()
    binaries = {binary["name"]: binary for binary in crate["binaries"]}
    if bin == None:
        if len(binaries) != 1:
            fail("{} has {} binaries; name the one this is with bin".format(crate["name"], len(binaries)))
        bin = binaries.keys()[0]
    binary = binaries[bin]
    crate_name = bin.replace("-", "_")
    own = []
    if crate["library"] != None:
        own = [":" + crate["name"]]
    native.rust_binary(
        name = name or (bin + "-bin" if bin == crate["name"] else bin),
        crate = crate_name,
        crate_root = binary["crate_root"],
        deps = _per_platform(crate, lambda entry: entry["binaries"].get(bin, []), own + extra_deps),
        edition = crate["edition"],
        env = _env(crate, crate_name, env),
        rustc_flags = _rustc_flags(crate, kwargs),
        srcs = _srcs(kwargs),
        visibility = kwargs.pop("visibility", ["PUBLIC"]),
        **kwargs,
    )

# One of the crate's examples, as a binary, named <example>-example:
# `./scripts/buck run //crates/beui:survey-example`. It is built the way cargo
# builds it, with the library and the dev-dependencies.
def cargo_example(example, extra_deps = [], env = {}, **kwargs):
    crate = _crate()
    examples = {entry["name"]: entry for entry in crate["examples"]}
    if example not in examples:
        fail("{} has no example {}; run ./scripts/buck, which regenerates buck/cargo/crates.bzl".format(crate["name"], example))
    crate_name = example.replace("-", "_")
    own = []
    if crate["library"] != None:
        own = [":" + crate["name"]]
    native.rust_binary(
        name = example + "-example",
        crate = crate_name,
        crate_root = examples[example]["crate_root"],
        deps = _per_platform(crate, lambda entry: entry["examples"].get(example, {}).get("deps", []), own + extra_deps),
        edition = crate["edition"],
        env = _env(crate, crate_name, env),
        features = _per_platform(crate, lambda entry: entry["examples"].get(example, {}).get("features", [])),
        rustc_flags = _rustc_flags(crate, kwargs),
        srcs = kwargs.pop("srcs", native.glob(["src/**/*.rs", "examples/**/*.rs"])),
        visibility = kwargs.pop("visibility", ["PUBLIC"]),
        **kwargs,
    )

# What the wasm macros in buck/wasm/defs.bzl need from a crate's Cargo.toml:
# the crate and its wasm dependencies, without the dev-dependencies and with.
def cargo_wasm_facts():
    crate = _crate()
    library = crate["library"]
    return struct(
        crate = library["crate"],
        crate_root = library["crate_root"],
        deps = _per_platform(crate, lambda entry: entry["deps"]),
        edition = crate["edition"],
        env = _env(crate, library["crate"], {}),
        features = _per_platform(crate, lambda entry: entry["features"]),
        test_deps = _per_platform(crate, lambda entry: entry["deps"] + entry["test_deps"]),
        test_features = _per_platform(crate, lambda entry: entry["test_features"]),
    )

# Every editor's package, which is every workspace crate under crates/editors:
# what the app stages as its plugins.
def editor_packages():
    return sorted([package for package in crates if package.startswith("crates/editors/")])

# Every game's package, which is every workspace crate under
# crates/tabletop-games/rules: what the games plugin stages as its data.
def game_packages():
    return sorted([package for package in crates if package.startswith("crates/tabletop-games/rules/")])
