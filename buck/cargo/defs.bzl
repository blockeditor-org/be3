load("@root//buck/platforms:profile.bzl", "dev_only")
load(":crates.bzl", "crates")

# The rules for a workspace crate, filled in from its Cargo.toml through
# crates.bzl, which ./scripts/buck generates from cargo's own plans. A BUCK file
# passes what Cargo.toml cannot say as arguments: extra_deps and env are added
# to what the macro works out, and anything else goes to the rule as it is.
# Where a crate's dependencies or features differ between platforms, the macro
# writes the select(); these are the constraints each plan is selected by.
_CONSTRAINTS = {
    "android-arm64": "root//buck/platforms:android_arm64_setting",
    "linux-arm64": "root//buck/platforms:linux_arm64_setting",
    "linux-x86_64": "DEFAULT",
    "macos-arm64": "root//buck/platforms:macos_arm64_setting",
    "macos-x86_64": "root//buck/platforms:macos_x86_64_setting",
    "wasi": "root//buck/platforms:wasi_setting",
    "wasi-guest": "root//buck/platforms:wasi_guest_setting",
    "wasm32": "prelude//os:none",
    "windows-arm64": "root//buck/platforms:windows_arm64_setting",
    "windows-x86_64": "root//buck/platforms:windows_x86_64_setting",
}

def _crate():
    package = native.package_name()
    if package not in crates:
        fail("{} is not a workspace member cargo knows about; run ./scripts/buck, which regenerates buck/cargo/crates.bzl".format(package))
    return crates[package]

# One value per platform, as a select() when they differ and a plain list when
# they do not. A crate cargo never builds for the host still needs a DEFAULT,
# and gets the first platform it is built for.
def _per_platform(crate, pick, extra = []):
    values = {}
    for platform, entry in crate["platforms"].items():
        values[_CONSTRAINTS[platform]] = sorted(pick(entry) + extra)
    if not values:
        return sorted(extra)
    distinct = []
    for value in values.values():
        if value not in distinct:
            distinct.append(value)
    if len(distinct) == 1:
        return distinct[0]
    if "DEFAULT" not in values:
        values["DEFAULT"] = values[sorted(values.keys())[0]]
    return select(values)

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
def cargo_test(name = "test", extra_deps = [], env = {}, **kwargs):
    crate = _crate()
    library = crate["library"]
    rustc_flags = _rustc_flags(crate, kwargs)
    if library["proc_macro"]:
        rustc_flags = rustc_flags + ["--extern", "proc_macro"]
    native.rust_test(
        name = name,
        crate = library["crate"],
        crate_root = library["crate_root"],
        deps = _per_platform(crate, lambda entry: entry["deps"] + entry["test_deps"], extra_deps),
        edition = crate["edition"],
        env = _env(crate, library["crate"], env),
        features = _per_platform(crate, lambda entry: entry["test_features"]),
        rustc_flags = rustc_flags,
        srcs = _srcs(kwargs),
        **kwargs,
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
# `./scripts/buck run //crates/beui:dock-example`. It is built the way cargo
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
