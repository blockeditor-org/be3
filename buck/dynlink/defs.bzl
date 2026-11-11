load("@root//buck/platforms:profile.bzl", "RUST_LOCAL")
load("@root//third-party/rust:crates.bzl", "third_party")

# Native Rust dynamic libraries for non-release Linux x86_64 builds: a few big
# Rust-ABI dylibs that every test harness and binary shares, instead of each
# linking every crate statically. See guides/app_libraries.md for the wasm
# libraries this is modelled on.
#
# A dylib's roots are rust_library targets. rustc links the roots and every
# crate they depend on into the one file, since the roots are loaded with
# --extern force:, and a binary compiled against the very same rlibs and told
# to load the dylib (--extern force:) takes those crates from it
# (rustc's Linkage::IncludedFromDylib) instead of linking its own copy.
# std is linked as the sysroot's libstd-*.so (-Cprefer-dynamic).

DynlibInfo = provider(fields = {
    "crate": str,
    "libs": list,  # the dylibs this one is linked against, which a binary loads too
    "lib": Artifact,
})

_LINUX_X86_64 = ["prelude//os:linux", "prelude//cpu:x86_64"]

_FORCE = ["-Zunstable-options", "-Cprefer-dynamic"]

# Every library: the roots it holds and the libraries it sits on, which the
# BUCK file defines and a binary is linked against. Order is not significant.

# The roots of base: every third-party library Linux x86_64 builds that no other
# one depends on, so that its closure is all of them, and none that only a build
# script uses. A crate Cargo.lock gains is in it without a word here.
#
# None of the crates that bring C code or link a system library, nor of those
# that depend on one (wasmtime, wgpu, winit, rustls, zstd, FreeType and the
# like, from the build scripts that use cc, cmake or pkg-config): the C code's
# symbols may be hidden, and the generic code of such a crate, which is
# compiled into whoever uses it, calls them, so a crate outside the library
# could not link; and a system library the library names has to be found by
# every test that loads it. A binary that uses one links it and what depends on
# it itself.
_C = ["cc-", "cmake-", "pkg-config-"]

# Crates that link a system library by name in their sources. A library
# naming one that is not in `readelf -d`'s NEEDED list of the build is a bug
# to be fixed by adding it.
_SYSTEM = ["xkbcommon-0.8.0"]

def _third_party_roots():
    libraries = {
        key: crate
        for key, crate in third_party.items()
        if "linux-x86_64" in crate["platforms"] and crate["library"] and not crate["library"]["proc_macro"]
    }
    users = {}
    for key, crate in libraries.items():
        for dep in crate["platforms"]["linux-x86_64"].get("deps", []):
            users.setdefault(dep.removeprefix(":"), []).append(key)
    unfit = {}
    pending = _SYSTEM + [
        key
        for key, crate in libraries.items()
        if crate["build_script"] and any([dep.removeprefix(":").startswith(tuple(_C)) for dep in crate["build_script"].get("deps", [])])
    ]
    for _ in range(len(libraries)):
        if not pending:
            break
        key = pending.pop()
        if key in unfit:
            continue
        unfit[key] = True
        pending.extend(users.get(key, []))
    used = {}
    for key, crate in third_party.items():
        if key in unfit:
            continue
        for plan in crate["platforms"].values():
            for dep in plan.get("deps", []):
                used[dep.removeprefix(":")] = True
        for dep in (crate["build_script"] or {}).get("deps", []):
            used[dep.removeprefix(":")] = True
    roots = []
    names = {}
    for key in sorted(libraries):
        name = libraries[key]["library"]["crate"]
        if key in used or key in unfit or name in names:
            # Two versions of a crate are two roots of one name, which
            # rustc takes only one of; the other stays out of the library.
            continue
        names[name] = True
        roots.append("//third-party/rust:" + key)
    return roots

DYNLIBS = {
    "base": {
        "deps": _third_party_roots(),
        "libs": [],
    },
}

# Crates whose tests do not compile with the libraries loaded: rustc loads every
# crate under them, and the impls those bring make an inference the test
# relies on ambiguous (be-type-terminal's `assert_eq!(f([]), vec![])`). The
# tests link everything themselves.
UNLINKED_TESTS = ["be-type-terminal"]

def _crate(name):
    return "dyn_" + name.replace("-", "_")

def _at_link(flags, otherwise = []):
    # Only Linux on x86_64 outside a release build, a LTO one and the HTML
    # core's, whose rlibs the libraries are not built from.
    return select({
        "DEFAULT": select({
            "DEFAULT": select({
                "DEFAULT": otherwise,
                "prelude//os:linux": select({
                    "DEFAULT": otherwise,
                    "prelude//cpu:x86_64": select({
                        "DEFAULT": flags,
                        "root//buck/constraints:html": otherwise,
                    }),
                }),
            }),
            "root//buck/constraints:lto": otherwise,
        }),
        "root//buck/constraints:release": otherwise,
    })

# What links a binary or test harness against every library, when it may be:
# the flags, and the libraries as dependencies with the force flag, which gives
# rustc the rlibs the libraries were linked from (it reads their metadata) as
# well as the libraries.
def dynlink_flags():
    return _at_link(_FORCE + ["-Clink-arg=-Wl,-rpath,$ORIGIN"])

# The directory of every library, for a program to stage beside its executable.
def dynlink_trees():
    return _at_link(["//buck/dynlink:libs"])

def dynlink_deps():
    return _at_link([("//buck/dynlink:{}-crate".format(name), ["force"]) for name in DYNLIBS])

# An LD_LIBRARY_PATH that adds where the libraries are to path ("" for none).
def dynlink_library_path(path):
    return _at_link(
        ((path + ":") if path else "") + "$(location //buck/dynlink:libs)",
        path or "",
    )

def dynlib(name, deps, libs = []):
    crate = _crate(name)
    native.rust_library(
        name = name + "-crate",
        crate = crate,
        crate_root = "lib.rs",
        srcs = ["lib.rs"],
        edition = "2021",
        flagged_deps = [(dep, ["force"]) for dep in deps] + [(lib + "-crate", ["force"]) for lib in libs],
        preferred_linkage = "shared",
        rustc_flags = _FORCE + [
            # Every symbol is exported, the C ones a crate's native library
            # brings too (FreeType's, say), which a crate outside the library
            # calls and rustc would hide.
            "-Clink-arg=-Wl,--version-script=$(location //buck/dynlink:exports.map)",
            "-Alinker-messages",
            # Every object of every archive too, so that the C code a crate's
            # native library holds is all in it, and not only what the library's own
            # code calls.
            "-Zpre-link-arg=-Wl,--whole-archive",
            "-Clink-arg=-Wl,--no-whole-archive",
            # ... which makes libgcc's and compiler_builtins' copies of the same
            # functions two definitions.
            "-Clink-arg=-Wl,--allow-multiple-definition",
        ],
        target_compatible_with = _LINUX_X86_64,
        **RUST_LOCAL
    )
    _dynlib(
        name = name,
        crate = crate,
        library = ":" + name + "-crate",
        libs = libs,
        target_compatible_with = _LINUX_X86_64,
    )

def _dynlib_impl(ctx: AnalysisContext) -> list[Provider]:
    lib = ctx.attrs.library[DefaultInfo].sub_targets["dylib"][DefaultInfo].default_outputs[0]
    libs = []
    for dep in ctx.attrs.libs:
        for under in dep[DynlibInfo].libs + [dep[DynlibInfo]]:
            if under not in libs:
                libs.append(under)
    return [
        DefaultInfo(default_output = lib),
        DynlibInfo(crate = ctx.attrs.crate, lib = lib, libs = libs),
    ]

_dynlib = rule(
    attrs = {
        "crate": attrs.string(),
        "library": attrs.dep(),
        "libs": attrs.list(attrs.dep(providers = [DynlibInfo]), default = []),
    },
    impl = _dynlib_impl,
)

# A directory holding every dylib and the sysroot's own (libstd, libtest):
# what LD_LIBRARY_PATH names for a binary linked against them.
def _dynlibs_impl(ctx: AnalysisContext) -> list[Provider]:
    out = ctx.actions.declare_output("lib", dir = True)
    libs = []
    for dep in ctx.attrs.libs:
        for info in dep[DynlibInfo].libs + [dep[DynlibInfo]]:
            if info not in libs:
                libs.append(info)
    ctx.actions.run(
        cmd_args(
            "sh",
            "-c",
            'out="$1"; sysroot="$2"; shift 2; mkdir -p "$out"; cp "$sysroot"/lib/rustlib/x86_64-unknown-linux-gnu/lib/*.so "$out"/; for lib; do cp "$lib" "$out"/; done',
            "--",
            out.as_output(),
            ctx.attrs._sysroot[DefaultInfo].default_outputs[0],
            [info.lib for info in libs],
        ),
        category = "dynlibs",
    )
    return [DefaultInfo(default_output = out), DynlibsInfo(libs = libs, dir = out)]

DynlibsInfo = provider(fields = {"libs": list, "dir": Artifact})

dynlibs = rule(
    attrs = {
        "libs": attrs.list(attrs.dep(providers = [DynlibInfo])),
        "_sysroot": attrs.default_only(attrs.dep(default = "root//buck/tools:rust")),
    },
    impl = _dynlibs_impl,
)
