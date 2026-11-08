# Which crate families may depend on which (guides/repository.md), checked for
# every workspace crate when its BUCK file loads (check_layers, which
# cargo_package() in buck/cargo/defs.bzl calls) from its dependencies in its
# facts (its PACKAGE), on every platform cargo planned it for. Only normal and build
# dependencies are checked: dev-dependencies, which end-to-end tests use to
# reach across the stack, are not.

# A crate's family is the one its name equals or starts with followed by "-".
# The order does not matter: no family is another followed by "-".
FAMILIES = [
    "be-app",
    "be-block",
    "be-host",
    "be-lib",
    "be-sdk",
    "be-server",
    "be-tool",
    "be-type",
    "be-wasm",
    "beui",
    "cvl2",
    "logicgame",
    "tabletop",
]

# What each family's crates may depend on: a crate whose name is an entry or
# starts with it followed by "-". A family not named here (be-tool) may depend
# on anything, and as no family names be-tool or cvl2, nothing else may depend
# on them.
LAYERS = {
    # Not be-app: each app is a module of its own.
    "be-app": ["be-sdk", "be-type", "be-block", "beui", "be-lib", "be-wasm", "logicgame", "tabletop"],
    # be-sdk-type-*: the type-module protocol the block stack runs types through.
    "be-block": ["be-block", "be-lib", "be-wasm", "be-sdk-type"],
    "be-host": [
        "be-host",
        "be-sdk",
        "be-type",
        "be-block",
        "be-server",
        "beui",
        "be-lib",
        "be-wasm",
        "logicgame",
        "tabletop",
    ],
    "be-lib": ["be-lib", "be-wasm"],
    "be-sdk": ["be-sdk", "be-block", "be-type", "beui", "be-lib", "be-wasm"],
    "be-server": ["be-server", "be-block", "be-lib", "be-wasm"],
    "be-type": ["be-type", "be-sdk-type", "be-block", "be-lib", "be-wasm", "logicgame", "tabletop"],
    # The guest ABI and running wasm guests: the base, under be-lib.
    "be-wasm": ["be-wasm"],
    "beui": ["beui", "be-lib", "be-wasm"],
    "cvl2": [],
    "logicgame": ["be-lib"],
    "tabletop": ["tabletop", "be-lib"],
}

# Crates that may depend on more than their family.
LAYER_EXTRAS = {
    # decisions/be_server_portal_ui_only_in_the_executable.md
    "be-server-portal-ui": ["beui"],
}

# Crates that may depend on less than their family: the app protocols stay
# independent of any GUI framework.
LAYER_DENY = {
    "be-sdk-api": ["beui"],
    "be-sdk-type-api": ["beui"],
}

# Test harnesses and benchmarks, which drive whole programs, are not checked.
LAYER_EXEMPT_SUFFIXES = ["-bench", "-tests", "-testing"]

def _matches(name, entry):
    return name == entry or name.startswith(entry + "-")

def _family(name):
    for family in FAMILIES:
        if _matches(name, family):
            return family
    return None

def _workspace_deps(crate):
    deps = {}
    for entry in crate["platforms"].values():
        for dep in entry["deps"] + [dep for bin in entry["binaries"].values() for dep in bin]:
            if dep.startswith("//crates/"):
                deps[dep.removeprefix("//crates/").split(":")[0]] = True
    return deps.keys()

def check_layers(crate):
    name = crate["name"]
    family = _family(name)
    if family == None:
        fail(
            "{} is in no crate family; name it with one of {} (guides/repository.md), or add its family to FAMILIES and LAYERS in buck/cargo/layers.bzl".format(
                name, ", ".join(FAMILIES)
            )
        )
    if family not in LAYERS or any([name.endswith(suffix) for suffix in LAYER_EXEMPT_SUFFIXES]):
        return
    allowed = LAYERS[family] + LAYER_EXTRAS.get(name, [])
    denied = LAYER_DENY.get(name, [])
    for dep in _workspace_deps(crate):
        if any([_matches(dep, entry) for entry in denied]) or not any([_matches(dep, entry) for entry in allowed]):
            fail(
                (
                    "{name} depends on {dep}, which its layer may not: a {family} crate may depend only on {allowed}{denied} "
                    + "(buck/cargo/layers.bzl, guides/repository.md). Move what {name} needs from {dep} into a crate it may use, "
                    + "move {name} into a family that may use {dep} by renaming it, or make {dep} a dev-dependency if only tests need it."
                ).format(
                    name = name,
                    allowed = ", ".join([entry + "-*" for entry in allowed]) or "nothing first-party",
                    denied = "".join([", and not on " + entry + "-*" for entry in denied]),
                    dep = dep,
                    family = family,
                )
            )
