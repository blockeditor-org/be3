#!/usr/bin/env python3
#
# Writes buck/cargo/crates.bzl: what each workspace crate's Cargo.toml says, in
# the terms a BUCK file needs, for each platform the build targets.
#
# What cargo builds depends on what it is asked to build: features are unified
# across one invocation, so beui has the window feature in a build of the app
# and only render in a build of the plugins. So the input is cargo's own plan
# for each of the invocations buck2 stands in for - `cargo test --unit-graph`
# for the host and the plugins, `cargo build --unit-graph` for the games - and
# a crate's dependencies and features on a platform are the ones cargo would
# have compiled it with there. ./scripts/buck run //:buckify runs this on a worker, next to
# reindeer; buck/cargo/defs.bzl is what reads the result.
#
# Usage:
#   generate.py METADATA.json PLATFORM=UNIT_GRAPH.json... > crates.bzl

import json
import os
import sys

LIBRARY_KINDS = {"lib", "rlib", "cdylib", "staticlib", "dylib", "proc-macro"}


def main():
    with open(sys.argv[1]) as file:
        metadata = json.load(file)
    graphs = {}
    for argument in sys.argv[2:]:
        name, path = argument.split("=", 1)
        with open(path) as file:
            graphs[name] = json.load(file)

    root = metadata["workspace_root"]
    members = {package["id"]: package for package in metadata["packages"]}

    def directory(package):
        return os.path.relpath(os.path.dirname(package["manifest_path"]), root)

    def label(package_id):
        if package_id in members:
            package = members[package_id]
            return "//{}:{}".format(directory(package), package["name"])
        name = package_id.split("#", 1)[1].split("@", 1)[0]
        return "//third-party/rust:{}".format(name)

    crates = {}
    for package in sorted(members.values(), key=directory):
        here = os.path.dirname(package["manifest_path"])
        library = None
        binaries = []
        examples = []
        for target in package["targets"]:
            root_path = os.path.relpath(target["src_path"], here)
            kinds = set(target["kind"])
            if "example" in kinds:
                examples.append({"crate_root": root_path, "name": target["name"]})
            elif kinds & LIBRARY_KINDS:
                library = {
                    "crate": target["name"].replace("-", "_"),
                    "crate_root": root_path,
                    "proc_macro": "proc-macro" in kinds,
                }
            elif "example" in kinds:
                # An example links the library and the dev-dependencies, the
                # way cargo builds one, with the package's features as they
                # stand in that build.
                entry["examples"][unit["target"]["name"]] = {
                    "deps": sorted(dependencies(unit)),
                    "features": sorted(unit["features"]),
                }
            elif "bin" in kinds:
                binaries.append({"crate_root": root_path, "name": target["name"]})
        crates[package["id"]] = {
            "binaries": sorted(binaries, key=lambda binary: binary["name"]),
            "edition": package["edition"],
            "examples": sorted(examples, key=lambda example: example["name"]),
            "library": library,
            "name": package["name"],
            "platforms": {},
            "version": package["version"],
        }

    for platform, graph in graphs.items():
        units = graph["units"]

        def dependencies(unit):
            found = set()
            for dependency in unit["dependencies"]:
                other = units[dependency["index"]]
                if "custom-build" in other["target"]["kind"]:
                    continue
                if other["pkg_id"] == unit["pkg_id"]:
                    continue
                found.add(label(other["pkg_id"]))
            return found

        for unit in units:
            crate = crates.get(unit["pkg_id"])
            if crate is None:
                continue
            kinds = set(unit["target"]["kind"])
            # A proc macro is compiled for the host whatever the build is for;
            # anything else counts on the platform only when it is built for it.
            if not (kinds & {"proc-macro"}) and graph_platform(unit) != platform_triple(platform, graph):
                continue
            entry = crate["platforms"].setdefault(
                platform,
                {"binaries": {}, "deps": [], "examples": {}, "features": [], "test_deps": [], "test_features": []},
            )
            if kinds & LIBRARY_KINDS and unit["mode"] == "build":
                entry["deps"] = sorted(dependencies(unit))
                entry["features"] = sorted(unit["features"])
            elif kinds & LIBRARY_KINDS and unit["mode"] == "test":
                entry["test_deps"] = sorted(dependencies(unit))
                entry["test_features"] = sorted(unit["features"])
            elif "example" in kinds:
                # An example links the library and the dev-dependencies, the
                # way cargo builds one, with the package's features as they
                # stand in that build.
                entry["examples"][unit["target"]["name"]] = {
                    "deps": sorted(dependencies(unit)),
                    "features": sorted(unit["features"]),
                }
            elif "bin" in kinds:
                # cargo test plans a binary as a test of itself, so its
                # dependencies include the dev-dependencies; those are taken
                # back out below, once the library's test is known.
                entry["binaries"][unit["target"]["name"]] = dependencies(unit)

    for crate in crates.values():
        for entry in crate["platforms"].values():
            dev_only = set(entry["test_deps"]) - set(entry["deps"])
            entry["test_deps"] = sorted(dev_only)
            entry["binaries"] = {name: sorted(deps - dev_only) for name, deps in entry["binaries"].items()}

    print("# @generated by ./scripts/buck from the workspace's Cargo.toml files.")
    print("# Do not edit by hand: buck/cargo/generate.py writes it, and the macros in")
    print("# buck/cargo/defs.bzl read it.")
    print("")
    print("crates = " + starlark({directory(members[key]): value for key, value in crates.items()}, 0))


def graph_platform(unit):
    return unit.get("platform")


def platform_triple(platform, graph):
    # The host build is not cross-compiled, so cargo reports its units with no
    # platform at all; the others name the triple they were asked for.
    triples = {unit.get("platform") for unit in graph["units"] if unit.get("platform")}
    return next(iter(triples)) if triples else None


def starlark(value, depth):
    indent = "    " * (depth + 1)
    closing = "    " * depth
    if value is None:
        return "None"
    if isinstance(value, bool):
        return "True" if value else "False"
    if isinstance(value, str):
        return json.dumps(value)
    if isinstance(value, list):
        if not value:
            return "[]"
        return "[\n" + "".join(indent + starlark(item, depth + 1) + ",\n" for item in value) + closing + "]"
    if isinstance(value, dict):
        if not value:
            return "{}"
        return "{\n" + "".join(
            indent + json.dumps(key) + ": " + starlark(item, depth + 1) + ",\n" for key, item in sorted(value.items())
        ) + closing + "}"
    raise TypeError(value)


main()
