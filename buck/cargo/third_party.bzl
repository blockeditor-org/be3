load("@prelude//rust:cargo_buildscript.bzl", "buildscript_run")
load("@prelude//rust:cargo_package.bzl", "cargo")
load("@root//buck/platforms:profile.bzl", "dev_only")
load(":crates.bzl", "third_party")

# The rules for every third-party crate a workspace crate depends on, from
# crates.bzl, which ./scripts/buck generates from cargo's own plans: one
# download of the crate, its library, and its build script with the action that
# runs it, named <name>-<version>. Features and dependencies are cargo's for
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

def _crate(key, crate, fixup):
    archive = key + ".crate"
    native.http_archive(
        name = archive,
        sha256 = crate["sha256"],
        size_bytes = crate["size_bytes"],
        strip_prefix = key,
        urls = ["https://static.crates.io/crates/{}/{}/download".format(crate["name"], crate["version"])],
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
        rustc_flags = dev_only(crate.get("profile_flags", [])) + rustc_flags,
        srcs = [":" + archive],
        visibility = ["PUBLIC"],
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
        srcs = [":" + archive],
        visibility = [],
    )
    run_env = {name: value for name, value in env.items() if name != "CARGO_MANIFEST_DIR"}
    if "links" in crate:
        run_env["CARGO_MANIFEST_LINKS"] = crate["links"]
    run_env.update(fixup.get("build_script_env", {}))
    buildscript_run(
        name = key + "-build-script-run",
        buildscript_rule = ":{}-build-script-build".format(key),
        env = run_env,
        package_name = crate["name"],
        platform = _platforms(crate, fixup, script = True),
        rustc_link_lib = True,
        rustc_link_search = True,
        version = crate["version"],
    )

def third_party_crates(fixups):
    for key, crate in third_party.items():
        _crate(key, crate, fixups.get(crate["name"], {}))
