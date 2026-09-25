# Turns cargo's own plans for the workspace into the facts the build is made
# of: for every third-party crate, the BUILD file of its repository, and for
# every workspace crate, what bazel/cargo/defs.bzl needs to write its rules.
#
# A plan is `cargo build` or `cargo test --unit-graph` for one platform, and
# each unit in it is one crate cargo would compile: its features and
# dependencies, exactly as cargo resolved them for that build. A unit cargo
# builds for the machine running the build - a proc macro, a build script and
# what they depend on - is recorded under linux_x86_64, since that is what the
# workers building everything are.

HOST = "linux_x86_64"

_LIBRARY_KINDS = ["lib", "rlib", "cdylib", "staticlib", "dylib", "proc-macro"]

def _is_library(kinds):
    for kind in kinds:
        if kind in _LIBRARY_KINDS:
            return True
    return False

def _relative(path, base):
    if not path.startswith(base + "/"):
        fail("{} is not inside {}".format(path, base))
    return path[len(base) + 1:]

def _dirname(path):
    return path.rsplit("/", 1)[0]

def repository_name(name, version):
    return "crate__{}-{}".format(name, version.replace("+", "-"))

def hub_label(name, version):
    return "@crates//:{}-{}".format(name, version.replace("+", "-"))

def parse_lock(text):
    """The checksum of every registry package in Cargo.lock, by name@version."""
    checksums = {}
    name = None
    version = None
    for line in text.splitlines():
        line = line.strip()
        if line == "[[package]]":
            name = None
            version = None
        elif line.startswith("name = "):
            name = line[len("name = "):].strip('"')
        elif line.startswith("version = "):
            version = line[len("version = "):].strip('"')
        elif line.startswith("checksum = "):
            checksums["{}@{}".format(name, version)] = line[len("checksum = "):].strip('"')
    return checksums

def _new_facts():
    return {
        "aliases": {},
        "deps": {},
        "features": {},
        "proc_macro_deps": {},
    }

def _add(facts, features, dependencies):
    for feature in features:
        facts["features"][feature] = True
    for label, extern, is_proc_macro, crate_name in dependencies:
        if is_proc_macro:
            facts["proc_macro_deps"][label] = True
        else:
            facts["deps"][label] = True
        if extern != crate_name:
            facts["aliases"][label] = extern

def _finish(facts):
    return {
        "aliases": {label: facts["aliases"][label] for label in sorted(facts["aliases"])},
        "deps": sorted(facts["deps"]),
        "features": sorted(facts["features"]),
        "proc_macro_deps": sorted(facts["proc_macro_deps"]),
    }

def generate(metadata, plans):
    """Returns (third_party, workspace).

    third_party maps name@version to the facts of a registry crate, and
    workspace maps a workspace crate's directory to its own; plans is a list of
    (platform, unit graph).
    """
    root = metadata["workspace_root"]
    packages = {package["id"]: package for package in metadata["packages"]}
    members = {id: True for id in metadata["workspace_members"]}

    def directory(package):
        return _relative(_dirname(package["manifest_path"]), root)

    def label(package, target):
        if package["id"] in members:
            return "//{}:{}".format(directory(package), package["name"])
        return hub_label(package["name"], package["version"])

    third_party = {}
    workspace = {}

    for id, package in packages.items():
        here = _dirname(package["manifest_path"])
        library = None
        build_script = None
        binaries = []
        examples = []
        for target in package["targets"]:
            kinds = target["kind"]
            entry = {
                "crate_root": _relative(target["src_path"], here),
                "edition": target.get("edition", package["edition"]),
                "name": target["name"],
            }
            if "example" in kinds:
                examples.append(entry)
            elif "custom-build" in kinds:
                build_script = entry
            elif _is_library(kinds):
                entry["crate"] = target["name"].replace("-", "_")
                entry["proc_macro"] = "proc-macro" in kinds
                library = entry
            elif "bin" in kinds:
                binaries.append(entry)
        common = {
            "build_script": build_script,
            "edition": package["edition"],
            "library": library,
            "links": package.get("links"),
            "name": package["name"],
            "platforms": {},
            "version": package["version"],
        }
        if id in members:
            common["binaries"] = sorted(binaries, key = lambda entry: entry["name"])
            common["directory"] = directory(package)
            common["examples"] = sorted(examples, key = lambda entry: entry["name"])
            workspace[id] = common
        else:
            common["build_script_deps"] = _new_facts()
            third_party[id] = common

    for platform, graph in plans:
        units = graph["units"]

        def dependencies(unit, units = units):
            found = []
            for dependency in unit["dependencies"]:
                other = units[dependency["index"]]
                kinds = other["target"]["kind"]
                if "custom-build" in kinds or other["pkg_id"] == unit["pkg_id"]:
                    continue
                package = packages[other["pkg_id"]]
                found.append((
                    label(package, other["target"]),
                    dependency["extern_crate_name"],
                    "proc-macro" in kinds,
                    other["target"]["name"].replace("-", "_"),
                ))
            return found

        for unit in units:
            id = unit["pkg_id"]
            kinds = unit["target"]["kind"]
            mode = unit["mode"]
            destination = HOST if unit["platform"] == None else platform
            if id in third_party:
                crate = third_party[id]
                if "custom-build" in kinds and mode == "build":
                    _add(crate["build_script_deps"], [], dependencies(unit))
                    continue
                if mode == "run-custom-build":
                    facts = crate["platforms"].setdefault(destination, {"library": _new_facts(), "build_script": _new_facts()})
                    _add(facts["build_script"], unit["features"], [])
                    continue
                if _is_library(kinds) and mode == "build":
                    facts = crate["platforms"].setdefault(destination, {"library": _new_facts(), "build_script": _new_facts()})
                    _add(facts["library"], unit["features"], dependencies(unit))
                continue

            if id not in workspace:
                continue
            crate = workspace[id]
            if "custom-build" in kinds or mode == "run-custom-build" or mode == "doctest":
                continue
            facts = crate["platforms"].setdefault(destination, {
                "binaries": {},
                "examples": {},
                "library": _new_facts(),
                "test": _new_facts(),
            })
            name = unit["target"]["name"]
            if _is_library(kinds) and mode == "build":
                _add(facts["library"], unit["features"], dependencies(unit))
            elif _is_library(kinds) and mode == "test":
                _add(facts["test"], unit["features"], dependencies(unit))
            elif "example" in kinds:
                _add(facts["examples"].setdefault(name, _new_facts()), unit["features"], dependencies(unit))
            elif "bin" in kinds:
                _add(facts["binaries"].setdefault(name, _new_facts()), unit["features"], dependencies(unit))

    for crate in third_party.values():
        crate["build_script_deps"] = _finish(crate["build_script_deps"])
        crate["platforms"] = {
            platform: {part: _finish(facts[part]) for part in facts}
            for platform, facts in sorted(crate["platforms"].items())
        }

    for crate in workspace.values():
        platforms = {}
        for platform, facts in sorted(crate["platforms"].items()):
            library = _finish(facts["library"])
            test = _finish(facts["test"])
            platforms[platform] = {
                "binaries": {name: _finish(binary) for name, binary in sorted(facts["binaries"].items())},
                "examples": {name: _finish(example) for name, example in sorted(facts["examples"].items())},
                "library": library,
                "test": test,
            }
        crate["platforms"] = platforms

    return (
        {"{}@{}".format(crate["name"], crate["version"]): crate for crate in third_party.values() if crate["platforms"]},
        {crate["directory"]: crate for crate in workspace.values()},
    )

def _starlark(value):
    """A value as Starlark source: JSON is Starlark for strings, lists and dicts."""
    if type(value) == "bool":
        return "True" if value else "False"
    if value == None:
        return "None"
    return json.encode(value)

starlark = _starlark

# A value per platform, as a select() when they differ and the value itself
# when they do not. A platform the crate is not built for takes the host's, or
# failing that the first there is, so that a query of the whole graph can
# still configure it.
def per_platform(values, settings, empty):
    if not values:
        return _starlark(empty)
    distinct = []
    for value in values.values():
        if value not in distinct:
            distinct.append(value)
    if len(distinct) == 1:
        return _starlark(distinct[0])
    branches = {settings[platform]: value for platform, value in values.items()}
    branches["//conditions:default"] = values.get(HOST, values[sorted(values)[0]])
    return "select({})".format(_starlark(branches))
