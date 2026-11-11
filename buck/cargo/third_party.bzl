load("@prelude//rust:cargo_buildscript.bzl", "buildscript_run")
load("@prelude//rust:cargo_package.bzl", "cargo")
load("@root//buck/platforms:cross.bzl", "PLAN_SETTINGS")
load("@root//buck/platforms:profile.bzl", "RELEASE_SYMBOL_FLAGS", "RUST_LOCAL", "dev_only")
load(":defs.bzl", "proc_macro_visibility")

# The rules for every third-party crate a workspace crate depends on, from
# third-party/rust/crates.bzl, which ./scripts/buckify generates from cargo's
# own plans: its download, <name>-<version>.crate (one for every crate in
# Cargo.lock, which third-party/rust/downloads.bzl pins and buckify hands to
# cargo), its library, and its build script with the action that runs it,
# named <name>-<version>. Features and dependencies are cargo's for
# each platform, keyed by the platform names the root PACKAGE maps a
# configuration to. What cargo's plans cannot say comes from
# third-party/rust/fixups.bzl.

def _env(crate, archive):
    version = crate["version"]
    core, _, pre = version.split("+")[0].partition("-")
    major, minor, patch = core.split(".")
    env = {
        "CARGO_MANIFEST_DIR": archive,
        "CARGO_PKG_AUTHORS": "",
        "CARGO_PKG_DESCRIPTION": "",
        "CARGO_PKG_HOMEPAGE": "",
        "CARGO_PKG_LICENSE": "",
        "CARGO_PKG_NAME": crate["name"],
        "CARGO_PKG_README": "",
        "CARGO_PKG_REPOSITORY": "",
        "CARGO_PKG_RUST_VERSION": "",
        "CARGO_PKG_VERSION": version,
        "CARGO_PKG_VERSION_MAJOR": major,
        "CARGO_PKG_VERSION_MINOR": minor,
        "CARGO_PKG_VERSION_PATCH": patch,
        "CARGO_PKG_VERSION_PRE": pre,
    }
    env.update(crate.get("env", {}))
    return env

# Each platform's features and dependencies, for the prelude's cargo macros,
# which select them by platform name.
def _platforms(crate, fixup, script = False):
    extra = fixup.get("features", {})
    platforms = {}
    for platform, facts in crate["platforms"].items():
        entry = {"features": sorted(facts.get("features", []) + extra.get(platform, []))}
        if not script:
            entry["deps"] = facts.get("deps", [])
            entry["named_deps"] = facts.get("named_deps", {})
        platforms[platform] = entry
    return platforms

# Third-party crates at opt-level 1 on the host outside release builds, as
# cargo's `[profile.dev.package."*"]` would give them: at 0 each of their small
# generic functions stays a function of its own with a symbol, and a test binary
# was hundreds of MB that every worker running it fetched. They are rarely
# compiled again, so the slower compile is paid once. Debug assertions and
# overflow checks stay on, as rustc turns them off above opt-level 0 and cargo's
# dev profile keeps them. First-party crates stay at 0: at 1 every compile on a
# verify's critical path took about twice as long.
THIRD_PARTY_OPT = select({
    "DEFAULT": select({
        "DEFAULT": ["-Copt-level=1", "-Cdebug-assertions=on", "-Coverflow-checks=on"],
        "prelude//cpu:wasm32": [],
    }),
    "root//buck/constraints:release": [],
})

# A fixup's rustc_flags for one plan's platforms only.
def _plan_flags(fixup):
    flags = fixup.get("plan_rustc_flags", {})
    if not flags:
        return []
    branches = {"DEFAULT": []}
    for plan, extra in flags.items():
        branches[PLAN_SETTINGS[plan]] = extra
    return select(branches)

def _crate(key, crate, fixup):
    archive = key + ".crate"
    # The download, configured for one platform whichever platform asks, so a
    # crate built for several unpacks once rather than once for each. Its
    # output is still named after the archive, which is where the crate's
    # sources sit beside its rules.
    unpacked = key + "-source"
    native.configured_alias(
        name = unpacked,
        actual = ":" + archive,
        platform = "root//buck/platforms:linux_x86_64",
        visibility = [],
    )
    overlay = {source: "{}/{}".format(archive, path) for path, source in fixup.get("overlay", {}).items()}
    env = _env(crate, archive)
    library = crate["library"]
    build_script = crate["build_script"]
    if not fixup.get("build_script", True):
        build_script = None

    rustc_flags = list(fixup.get("rustc_flags", []))
    library_env = dict(env)
    library_env["CARGO_CRATE_NAME"] = library["crate"]
    if build_script:
        run = key + "-build-script-run"
        library_env["OUT_DIR"] = "$(location :{}[out_dir])".format(run)
        rustc_flags.append("@$(location :{}[rustc_flags])".format(run))

    cargo.rust_library(
        name = key,
        crate = library["crate"],
        crate_root = "{}/{}".format(archive, library["crate_root"]),
        deps = fixup.get("deps", []),
        edition = library["edition"],
        env = library_env,
        mapped_srcs = overlay,
        platform = _platforms(crate, fixup),
        proc_macro = library["proc_macro"],
        rustc_flags = dev_only(crate.get("profile_flags", [])) + RELEASE_SYMBOL_FLAGS + THIRD_PARTY_OPT + rustc_flags + _plan_flags(fixup),
        srcs = [":" + unpacked],
        **(proc_macro_visibility(library["proc_macro"], {}) | RUST_LOCAL),
    )

    if not build_script:
        return
    script_env = dict(env)
    script_env["CARGO_CRATE_NAME"] = "build_script_build"
    cargo.rust_binary(
        name = key + "-build-script-build",
        crate = "build_script_build",
        crate_root = "{}/{}".format(archive, build_script["crate_root"]),
        deps = build_script.get("deps", []),
        edition = build_script["edition"],
        env = script_env,
        mapped_srcs = overlay,
        named_deps = build_script.get("named_deps", {}),
        platform = _platforms(crate, fixup, script = True),
        srcs = [":" + unpacked],
        visibility = [],
        **RUST_LOCAL,
    )
    run_env = {name: value for name, value in env.items() if name != "CARGO_MANIFEST_DIR"}
    if "links" in crate:
        run_env["CARGO_MANIFEST_LINKS"] = crate["links"]
    run_env.update(fixup.get("build_script_env", {}))
    buildscript_run(
        name = key + "-build-script-run",
        buildscript_rule = ":{}-build-script-build".format(key),
        env = run_env,
        manifest_dir = ":" + unpacked,
        package_name = crate["name"],
        platform = _platforms(crate, fixup, script = True),
        rustc_link_lib = True,
        rustc_link_search = True,
        version = crate["version"],
    )

def third_party_crates(third_party, downloads, fixups):
    for key, download in downloads.items():
        native.http_archive(
            name = key + ".crate",
            sha256 = download["sha256"],
            size_bytes = download["size_bytes"],
            strip_prefix = key,
            urls = ["https://static.crates.io/crates/{}/{}/download".format(download["name"], download["version"])],
        )
    for key, crate in third_party.items():
        _crate(key, crate, fixups.get(crate["name"], {}))
