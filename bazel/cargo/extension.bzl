# The third-party crates, and what the workspace's own crates are made of,
# from cargo's own plans for the workspace.
#
# cargo is asked, for every platform the workspace is built for, what it would
# build (bazel/cargo/generate.bzl says how that becomes rules): a repository
# per registry crate, downloaded from crates.io and checked against the
# checksum Cargo.lock has for it, and @crates, which names each of them and
# holds crates.bzl, the facts bazel/cargo/defs.bzl writes a workspace crate's
# rules from. What cargo needs to plan - the manifests, Cargo.lock, the
# fixups and the paths cargo discovers targets at - are this extension's
# inputs, and MODULE.bazel.lock records what it made from them, so the plans are
# only made again, by whoever changes one of those, and everyone else reads
# them from the lockfile.

load("//bazel/platforms:defs.bzl", "PLATFORMS")
load("//third-party/rust:fixups.bzl", "FIXUPS")
load(":generate.bzl", "generate", "hub_label", "parse_lock", "per_platform", "repository_name", "starlark")

# The Rust cargo plans with: rust-toolchain.toml's, for the machine running
# Bazel. --unit-graph is unstable, which RUSTC_BOOTSTRAP allows on a stable
# cargo; nothing is compiled.
_RUST_VERSION = "1.98.0"

_RUST_SHA256S = {
    "cargo-aarch64-apple-darwin": "2c2a8bbf3cba4353c0ca2cf1ba8280603f3ca82ceebb538fee4a1a987147f743",
    "cargo-aarch64-pc-windows-msvc": "6d939ba34b790d4e03cdaca6c1568d9bc14cdb7b06c29c6213c266ea70478b6b",
    "cargo-aarch64-unknown-linux-gnu": "5784379d73ac881d15a9e67eed2882cd58c747c276221c23cdf9aff37e015ff6",
    "cargo-x86_64-apple-darwin": "3526be8588e7f80cf0604ce184dd8af89798786e389d5deea4ae4ffe2f104265",
    "cargo-x86_64-pc-windows-msvc": "c781bbfd0d0859349a14f82c04073521c036f7ada59c0fd14ef0722d035bb361",
    "cargo-x86_64-unknown-linux-gnu": "2f512d170d3dd23e16ababcda32ee2e6d5172d861a7af1f504e0b1e270cafab9",
    "rustc-aarch64-apple-darwin": "287edbc2e285b9c23ef7b085413b90cb8539909eda9c4b49f2a55ec0b52819d4",
    "rustc-aarch64-pc-windows-msvc": "5c47ea5a6b04ac437530be856d65d3aac04e4e54c4668e632f7ed203570e4ead",
    "rustc-aarch64-unknown-linux-gnu": "00590657f2356d7163ca5ef295283523974c340fa21bb94b420ce794f29b358c",
    "rustc-x86_64-apple-darwin": "c82d8f536955a9d6fc4465637fce5dcacf1d3913a98b5eb7edd9ead5a8b3f509",
    "rustc-x86_64-pc-windows-msvc": "a29cdefc01c747e15270ec64878adba4b0a88afaa7f19e3abf9231106fbd381b",
    "rustc-x86_64-unknown-linux-gnu": "0e37cb339f447fc44d6d781073bacacebfdc5612f2600e4c7e84c266f5f3aced",
}

# The builds Bazel stands in for, as cargo would be asked for them: the host
# (block-app with full, without the wasm-only crates), block-app for the web,
# the plugins (a plan of their own: they want a different wgpu), the games and
# the gpu shim, and the host's crates again for each other native platform.
# `cargo test` where there are tests, so the plan has the dev-dependencies.
def _plans(editors, games):
    plugins = []
    excluded = []
    for editor in editors:
        plugins.extend(["-p", editor])
        excluded.extend(["--exclude", editor])
    host = ["--workspace"] + excluded + ["--exclude", "block-editor-plugin", "--exclude", "block-gpu-shim", "--features", "block-app/full"]
    plans = []
    for name, (_, triple) in PLATFORMS.items():
        if name == "wasi":
            plans.append((name, triple, ["build", "-p", "block-app", "--lib", "--features", "block-app/full"]))
        elif name == "wasi_guest":
            plans.append((name, triple, ["test"] + plugins + ["-p", "block-editor-plugin"]))
        elif name == "wasm32":
            plans.append((name, triple, ["build"] + [flag for game in games for flag in ["-p", game]] + ["-p", "block-gpu-shim"]))
        else:
            plans.append((name, triple, ["test"] + host))
    return plans

def _host_triple(os):
    arch = {"aarch64": "aarch64", "amd64": "x86_64", "arm64": "aarch64", "x86_64": "x86_64"}.get(os.arch)
    name = os.name.lower()
    if name.startswith("linux"):
        system = "unknown-linux-gnu"
    elif name.startswith("mac"):
        system = "apple-darwin"
    elif name.startswith("windows"):
        system = "pc-windows-msvc"
    else:
        system = None
    if not arch or not system:
        fail("No Rust toolchain is pinned for {} on {}".format(os.name, os.arch))
    return "{}-{}".format(arch, system)

def _toolchain(mctx):
    triple = _host_triple(mctx.os)
    tools = {}
    for component in ["cargo", "rustc"]:
        mctx.download_and_extract(
            url = "https://static.rust-lang.org/dist/{}-{}-{}.tar.xz".format(component, _RUST_VERSION, triple),
            sha256 = _RUST_SHA256S["{}-{}".format(component, triple)],
            output = "rust/" + component,
            stripPrefix = "{}-{}-{}/{}".format(component, _RUST_VERSION, triple, component),
        )
        suffix = ".exe" if "windows" in triple else ""
        tools[component] = mctx.path("rust/{}/bin/{}{}".format(component, component, suffix))
    return tools

_PASSED_ENVIRONMENT = [
    "CARGO_HTTP_CAINFO",
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
    "SSL_CERT_DIR",
    "SSL_CERT_FILE",
    "https_proxy",
    "http_proxy",
    "no_proxy",
]

def _cargo(mctx, tools, root, arguments):
    environment = {
        "CARGO_HOME": str(mctx.path("cargo-home")),
        "CARGO_TARGET_DIR": str(mctx.path("cargo-target")),
        "RUSTC": str(tools["rustc"]),
        "RUSTC_BOOTSTRAP": "1",
    }
    for name in _PASSED_ENVIRONMENT:
        value = mctx.os.environ.get(name)
        if value:
            environment[name] = value
    result = mctx.execute(
        [str(tools["cargo"])] + arguments + ["--locked", "--manifest-path", str(root.get_child("Cargo.toml"))],
        environment = environment,
        working_directory = str(root),
        timeout = 1800,
        quiet = True,
    )
    if result.return_code != 0:
        fail("cargo {} failed:\n{}".format(" ".join(arguments), result.stderr))
    return json.decode(result.stdout.strip().splitlines()[-1])

def _watch_layout(mctx, root, crate):
    base = root.get_child(crate["directory"])
    mctx.watch(base)
    for directory in ["src", "src/bin", "examples", "tests", "benches"]:
        mctx.watch(base.get_child(directory))

def _settings():
    return {name: "@@//bazel/platforms:{}_setting".format(name) for name in PLATFORMS}

_EXCLUDED = ["BUILD", "BUILD.bazel", "WORKSPACE", "WORKSPACE.bazel", "MODULE.bazel", "REPO.bazel"]

# links is the crates with a build script and a links key, whose build
# script's metadata the build scripts of the crates depending on them are given
# as DEP_<LINKS>_<KEY>.
def _crate_build_file(crate, fixup, links):
    settings = _settings()
    lines = [
        'load("@rules_rust//cargo:defs.bzl", "cargo_build_script")',
        'load("@rules_rust//rust:defs.bzl", "rust_library", "rust_proc_macro")',
        "",
        "package(default_visibility = [\"//visibility:public\"])",
        "",
    ]
    library = crate["library"]
    platforms = crate["platforms"]

    def pick(part, field, extra = {}):
        values = {}
        for platform, facts in platforms.items():
            value = facts[part][field]
            if type(value) == "list":
                value = sorted({item: True for item in value + extra.get(platform, [])}.keys())
            values[platform] = value
        return values

    build_script = crate["build_script"]
    if fixup.get("build_script", True) == False:
        build_script = None
    extra_features = fixup.get("features", {})
    deps = per_platform(pick("library", "deps"), settings, [])
    extra_deps = [":build_script"] if build_script else []
    extra_deps = extra_deps + fixup.get("deps", [])
    if extra_deps:
        deps = "{} + {}".format(deps, starlark(extra_deps))
    rustc_flags = ["--cap-lints=allow"] + fixup.get("rustc_flags", [])
    lines.extend([
        "{}(".format("rust_proc_macro" if library["proc_macro"] else "rust_library"),
        "    name = {},".format(starlark(crate["name"])),
        "    aliases = {},".format(per_platform(pick("library", "aliases"), settings, {})),
        "    compile_data = glob([\"**\"], exclude = [\"**/*.rs\"] + {}, allow_empty = True),".format(starlark(_EXCLUDED)),
        "    crate_features = {},".format(per_platform(pick("library", "features", extra_features), settings, [])),
        "    crate_name = {},".format(starlark(library["crate"])),
        "    crate_root = {},".format(starlark(library["crate_root"])),
        "    deps = {},".format(deps),
        "    edition = {},".format(starlark(library["edition"])),
        "    proc_macro_deps = {},".format(per_platform(pick("library", "proc_macro_deps"), settings, [])),
        "    rustc_env = {},".format(starlark(fixup.get("rustc_env", {}))),
        "    rustc_flags = {},".format(starlark(rustc_flags)),
        "    srcs = glob([\"**/*.rs\"], allow_empty = True),",
        "    version = {},".format(starlark(crate["version"])),
        ")",
        "",
    ])
    if build_script:
        script_deps = crate["build_script_deps"]
        link_deps = []
        for platform_deps in pick("library", "deps").values():
            for dep in platform_deps:
                if dep in links and dep not in link_deps:
                    link_deps.append(dep)
        lines.extend([
            "cargo_build_script(",
            "    name = \"build_script\",",
            "    aliases = {},".format(starlark(script_deps["aliases"])),
            "    build_script_env = {},".format(starlark(fixup.get("build_script_env", {}))),
            "    compile_data = glob([\"**\"], exclude = [\"**/*.rs\"] + {}, allow_empty = True),".format(starlark(_EXCLUDED)),
            "    crate_features = {},".format(per_platform(pick("build_script", "features", extra_features), settings, [])),
            "    crate_name = \"build_script_build\",",
            "    crate_root = {},".format(starlark(build_script["crate_root"])),
            "    data = glob([\"**\"], exclude = {}, allow_empty = True) + {},".format(starlark(_EXCLUDED), starlark(fixup.get("build_script_data", []))),
            "    deps = {},".format(starlark(script_deps["deps"])),
            "    edition = {},".format(starlark(build_script["edition"])),
            "    link_deps = {},".format(starlark(sorted(link_deps))),
            "    links = {},".format(starlark(crate["links"])),
            "    pkg_name = {},".format(starlark(crate["name"])),
            "    proc_macro_deps = {},".format(starlark(script_deps["proc_macro_deps"])),
            "    rustc_flags = [\"--cap-lints=allow\"],",
            "    srcs = glob([\"**/*.rs\"], allow_empty = True),",
            "    tools = {},".format(starlark(fixup.get("build_script_tools", []))),
            "    version = {},".format(starlark(crate["version"])),
            ")",
            "",
        ])
    if "build_file_content" in fixup:
        lines.append(fixup["build_file_content"])
    return "\n".join(lines)

def _crate_repository_impl(rctx):
    rctx.download_and_extract(
        url = "https://static.crates.io/crates/{0}/{0}-{1}.crate".format(rctx.attr.crate, rctx.attr.version),
        sha256 = rctx.attr.sha256,
        type = "tar.gz",
        stripPrefix = "{}-{}".format(rctx.attr.crate, rctx.attr.version),
    )
    for path, label in rctx.attr.overlay.items():
        rctx.file(path, rctx.read(label), legacy_utf8 = False)
    for name in ["BUILD", "BUILD.bazel", "WORKSPACE", "WORKSPACE.bazel", "MODULE.bazel", "REPO.bazel"]:
        if rctx.path(name).exists:
            rctx.delete(name)
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", rctx.attr.build_file_content)
    return rctx.repo_metadata(reproducible = True)

_crate_repository = repository_rule(
    implementation = _crate_repository_impl,
    attrs = {
        "build_file_content": attr.string(),
        "crate": attr.string(),
        "overlay": attr.string_keyed_label_dict(),
        "sha256": attr.string(),
        "version": attr.string(),
    },
)

def _hub_impl(rctx):
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", rctx.attr.build_file_content)
    rctx.file("crates.bzl", rctx.attr.crates)

_hub = repository_rule(
    implementation = _hub_impl,
    attrs = {
        "build_file_content": attr.string(),
        "crates": attr.string(),
    },
)

def _members(text):
    """The paths of the workspace's members, from the root Cargo.toml's [workspace] members."""
    members = []
    inside = False
    for line in text.splitlines():
        line = line.strip()
        if line.startswith("members"):
            inside = True
        elif inside and line.startswith("]"):
            break
        elif inside and line.startswith('"'):
            members.append(line.strip(",").strip('"'))
    return members

def _impl(mctx):
    root = mctx.path(Label("//:Cargo.toml")).dirname
    lock = parse_lock(mctx.read(Label("//:Cargo.lock")))
    members = _members(mctx.read(Label("//:Cargo.toml")))
    for member in members:
        mctx.read(root.get_child(member).get_child("Cargo.toml"))

    tools = _toolchain(mctx)
    metadata = _cargo(mctx, tools, root, ["metadata", "--format-version", "1", "--all-features"])
    editors = []
    games = []
    for package in metadata["packages"]:
        if package["id"] not in metadata["workspace_members"]:
            continue
        manifest = package["manifest_path"]
        if "/crates/editors/" in manifest:
            editors.append(package["name"])
        if "/crates/tabletop_games/rules/" in manifest:
            games.append(package["name"])

    plans = []
    for name, triple, arguments in _plans(sorted(editors), sorted(games)):
        mode = arguments[0]
        graph = _cargo(mctx, tools, root, [mode, "--unit-graph", "-Z", "unstable-options", "--target", triple] + arguments[1:])
        plans.append((name, graph))

    third_party, workspace = generate(metadata, plans)
    for crate in workspace.values():
        _watch_layout(mctx, root, crate)

    links = {}
    for crate in third_party.values():
        if crate["links"] and crate["build_script"] and FIXUPS.get(crate["name"], {}).get("build_script", True):
            links[hub_label(crate["name"], crate["version"])] = True

    aliases = []
    for key, crate in sorted(third_party.items()):
        repository = repository_name(crate["name"], crate["version"])
        if key not in lock:
            fail("Cargo.lock has no checksum for {}".format(key))
        fixup = FIXUPS.get(crate["name"], {})
        _crate_repository(
            name = repository,
            build_file_content = _crate_build_file(crate, fixup, links),
            crate = crate["name"],
            overlay = fixup.get("overlay", {}),
            sha256 = lock[key],
            version = crate["version"],
        )
        aliases.append('alias(\n    name = "{}",\n    actual = "@{}//:{}",\n)\n'.format(
            hub_label(crate["name"], crate["version"]).removeprefix("@crates//:"),
            repository,
            crate["name"],
        ))

    _hub(
        name = "crates",
        build_file_content = 'package(default_visibility = ["//visibility:public"])\n\nexports_files(["crates.bzl"])\n\n' + "\n".join(aliases),
        crates = "# What cargo plans for each workspace crate, per platform: bazel/cargo/extension.bzl\n# writes it and bazel/cargo/defs.bzl reads it.\n\ncrates = json.decode(r\'\'\'{}\'\'\')\n".format(json.encode_indent(workspace)),
    )
    return mctx.extension_metadata(reproducible = False)

cargo = module_extension(implementation = _impl)
