# The rules for a workspace crate, filled in from its Cargo.toml through
# @crates//:crates.bzl, which bazel/cargo/extension.bzl makes from cargo's own
# plans. A BUILD file passes what Cargo.toml cannot say as arguments:
# extra_deps and env are added to what the macro works out, and anything else
# goes to the rule as it is. Where a crate's dependencies or features differ
# between platforms, the macro writes the select().

load("@crates//:crates.bzl", "crates")
load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_library", "rust_proc_macro", "rust_shared_library", "rust_test")

_SETTINGS = {
    "android_arm64": "//bazel/platforms:android_arm64_setting",
    "linux_arm64": "//bazel/platforms:linux_arm64_setting",
    "linux_x86_64": "//bazel/platforms:linux_x86_64_setting",
    "macos_arm64": "//bazel/platforms:macos_arm64_setting",
    "macos_x86_64": "//bazel/platforms:macos_x86_64_setting",
    "wasi": "//bazel/platforms:wasi_setting",
    "wasi_guest": "//bazel/platforms:wasi_guest_setting",
    "wasm32": "//bazel/platforms:wasm32_setting",
    "windows_arm64": "//bazel/platforms:windows_arm64_setting",
    "windows_x86_64": "//bazel/platforms:windows_x86_64_setting",
}

# What cargo's dev profile and the workspace's lints add to a first-party
# crate: line tables in a dev build, and every warning an error, the way
# `cargo clippy -- -D warnings` fails on one. Third-party crates are built
# with their lints capped (bazel/cargo/extension.bzl).
#
# A Mac's debug info is left in the objects rather than packed, which would run
# dsymutil after the link: that is Apple's, and not something a Linux worker has.
_RUSTC_FLAGS = ["-Dwarnings"] + select({
    Label("//bazel/constraints:release"): [],
    "//conditions:default": ["-Cdebuginfo=line-tables-only"],
}) + select({
    "@platforms//os:macos": ["-Csplit-debuginfo=unpacked"],
    "//conditions:default": [],
})

def _crate():
    package = native.package_name()
    if package not in crates:
        fail("{} is not a workspace member cargo knows about: add it to the workspace's Cargo.toml".format(package))
    return crates[package]

def _per_platform(crate, pick, extra = [], empty = []):
    """One value per platform, as a select() when they differ."""
    values = {}
    for platform, entry in crate["platforms"].items():
        value = pick(entry)
        if value == None:
            continue
        if type(value) == "list":
            value = sorted({item: True for item in value + extra}.keys())
        values[platform] = value
    if not values:
        return sorted(extra) if type(empty) == "list" else empty
    distinct = []
    for value in values.values():
        if value not in distinct:
            distinct.append(value)
    if len(distinct) == 1:
        return distinct[0]
    branches = {Label(_SETTINGS[platform]): value for platform, value in values.items()}
    branches["//conditions:default"] = values.get("linux_x86_64", values[sorted(values)[0]])
    return select(branches)

def _facts(crate, part, extra_deps = []):
    """deps, proc_macro_deps, aliases and features of one of a crate's parts, per platform."""

    def field(name, extra = [], empty = []):
        return _per_platform(crate, lambda entry: part(entry)[name] if part(entry) else None, extra, empty)

    return struct(
        aliases = field("aliases", empty = {}),
        deps = field("deps", extra_deps),
        features = field("features"),
        proc_macro_deps = field("proc_macro_deps"),
    )

def _env(crate, env):
    base = {
        "CARGO_MANIFEST_DIR": native.package_name(),
        "CARGO_PKG_NAME": crate["name"],
    }
    base.update(env)
    return base

def _srcs(kwargs, default = ["src/**/*.rs"]):
    return kwargs.pop("srcs", native.glob(default, allow_empty = True))

def _flags(kwargs):
    return _RUSTC_FLAGS + kwargs.pop("rustc_flags", [])

def cargo_library(name = None, extra_deps = [], env = {}, shared = False, **kwargs):
    """The crate's library, named after the package; with shared, its cdylib."""
    crate = _crate()
    library = crate["library"]
    facts = _facts(crate, lambda entry: entry["library"], extra_deps)
    rule = rust_proc_macro if library["proc_macro"] else rust_shared_library if shared else rust_library
    arguments = {}
    if not library["proc_macro"]:
        arguments["crate_features"] = facts.features
    rule(
        name = name or crate["name"],
        aliases = facts.aliases,
        crate_name = library["crate"],
        crate_root = library["crate_root"],
        deps = facts.deps,
        edition = library["edition"],
        proc_macro_deps = facts.proc_macro_deps,
        rustc_env = _env(crate, env),
        rustc_flags = _flags(kwargs),
        srcs = _srcs(kwargs),
        version = crate["version"],
        visibility = kwargs.pop("visibility", ["//visibility:public"]),
        **(arguments | kwargs)
    )

def cargo_test(name = "test", extra_deps = [], env = {}, **kwargs):
    """The library's own tests, which are #[cfg(test)] modules in its sources, with the dev-dependencies added."""
    crate = _crate()
    library = crate["library"]
    facts = _facts(crate, lambda entry: entry["test"], extra_deps)
    rustc_flags = _flags(kwargs)
    if library["proc_macro"]:
        rustc_flags = rustc_flags + ["--extern", "proc_macro"]
    runtime_env = {"CARGO_MANIFEST_DIR": native.package_name()}
    runtime_env.update(kwargs.pop("runtime_env", {}))
    rust_test(
        name = name,
        aliases = facts.aliases,
        crate_features = facts.features,
        crate_name = library["crate"],
        crate_root = library["crate_root"],
        deps = facts.deps,
        edition = library["edition"],
        env = runtime_env,
        proc_macro_deps = facts.proc_macro_deps,
        rustc_env = _env(crate, env),
        rustc_flags = rustc_flags,
        srcs = _srcs(kwargs),
        version = crate["version"],
        **kwargs
    )

def cargo_binary(bin = None, name = None, extra_deps = [], env = {}, **kwargs):
    """One of the crate's [[bin]] targets. A binary named like its package is <name>-bin, since the library already has the name."""
    crate = _crate()
    binaries = {binary["name"]: binary for binary in crate["binaries"]}
    if bin == None:
        if len(binaries) != 1:
            fail("{} has {} binaries; name the one this is with bin".format(crate["name"], len(binaries)))
        bin = binaries.keys()[0]
    binary = binaries[bin]
    own = [":" + crate["name"]] if crate["library"] else []
    facts = _facts(crate, lambda entry: entry["binaries"].get(bin), own + extra_deps)
    rust_binary(
        name = name or (bin + "-bin" if bin == crate["name"] else bin),
        aliases = facts.aliases,
        crate_features = facts.features,
        crate_name = bin.replace("-", "_"),
        crate_root = binary["crate_root"],
        deps = facts.deps,
        edition = binary["edition"],
        proc_macro_deps = facts.proc_macro_deps,
        rustc_env = _env(crate, env),
        rustc_flags = _flags(kwargs),
        srcs = _srcs(kwargs),
        version = crate["version"],
        visibility = kwargs.pop("visibility", ["//visibility:public"]),
        **kwargs
    )

def cargo_example(example, extra_deps = [], env = {}, **kwargs):
    """One of the crate's examples, as a binary named <example>-example, built the way cargo builds it."""
    crate = _crate()
    examples = {entry["name"]: entry for entry in crate["examples"]}
    if example not in examples:
        fail("{} has no example {}".format(crate["name"], example))
    own = [":" + crate["name"]] if crate["library"] else []
    facts = _facts(crate, lambda entry: entry["examples"].get(example), own + extra_deps)
    rust_binary(
        name = example + "-example",
        aliases = facts.aliases,
        crate_features = facts.features,
        crate_name = example.replace("-", "_"),
        crate_root = examples[example]["crate_root"],
        deps = facts.deps,
        edition = examples[example]["edition"],
        proc_macro_deps = facts.proc_macro_deps,
        rustc_env = _env(crate, env),
        rustc_flags = _flags(kwargs),
        srcs = _srcs(kwargs, ["src/**/*.rs", "examples/**/*.rs"]),
        version = crate["version"],
        visibility = kwargs.pop("visibility", ["//visibility:public"]),
        **kwargs
    )

def cargo_wasm_facts():
    """What the wasm macros in bazel/wasm/defs.bzl need from a crate's Cargo.toml."""
    crate = _crate()
    library = crate["library"]
    facts = _facts(crate, lambda entry: entry["library"])
    test = _facts(crate, lambda entry: entry["test"])
    return struct(
        aliases = facts.aliases,
        crate = library["crate"],
        crate_root = library["crate_root"],
        deps = facts.deps,
        edition = library["edition"],
        env = _env(crate, {}),
        features = facts.features,
        proc_macro_deps = facts.proc_macro_deps,
        rustc_flags = _RUSTC_FLAGS,
        test = test,
        version = crate["version"],
    )

def editor_packages():
    """Every editor's package, which is every workspace crate under crates/editors: what the app stages as its plugins."""
    return sorted([package for package in crates if package.startswith("crates/editors/")])
