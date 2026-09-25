load("@rules_rust//rust:defs.bzl", "rust_binary", "rust_shared_library")
load("//bazel/cargo:defs.bzl", "cargo_wasm_facts")

# WebAssembly modules, depended on from targets that are not WebAssembly: the
# app and the native tests only load a game or a plugin as a module, so the
# module is a dependency in another configuration, which these transitions give
# it. The compilation mode - Cargo's profile - is kept. A game's test says
# `rustc_env = {"GAME_WASM": "$(execpath :module)"}`.
def _to(platform):
    def _impl(_settings, _attr):
        return {"//command_line_option:platforms": str(platform)}

    return transition(
        implementation = _impl,
        inputs = [],
        outputs = ["//command_line_option:platforms"],
    )

wasm32_transition = _to(Label("//bazel/platforms:wasm32"))

# The plugins' wasi. The app's is the same triple, told apart by a constraint
# (bazel/constraints) so that each gets its own wgpu.
wasi_guest_transition = _to(Label("//bazel/platforms:wasi_guest"))

wasi_app_transition = _to(Label("//bazel/platforms:wasi"))

# A cdylib's output is the .wasm; this names it as the module it is, which for
# a plugin is the entry point its manifest names.
def _wasm_module_impl(ctx):
    library = ctx.attr.library[0][DefaultInfo].files.to_list()
    wasm = [file for file in library if file.extension == "wasm"]
    if len(wasm) != 1:
        fail("{} is not one wasm module".format(ctx.attr.library[0].label))
    module = ctx.actions.declare_file((ctx.attr.module or ctx.label.name) + ".wasm")
    ctx.actions.symlink(output = module, target_file = wasm[0])
    return [DefaultInfo(files = depset([module]))]

def _wasm_module(transition):
    return rule(
        implementation = _wasm_module_impl,
        attrs = {
            "library": attr.label(cfg = transition),
            "module": attr.string(),
        },
    )

wasm32_module = _wasm_module(wasm32_transition)

wasi_module = _wasm_module(wasi_guest_transition)

wasi_app_module = _wasm_module(wasi_app_transition)

# rustc links a cdylib with --no-entry, which strips the symbols a host needs to
# lay out a thread's storage and leaves nothing to run libc's constructors; the
# host does both itself, so a guest exports them.
PLUGIN_EXPORTS = [
    "-Clink-arg=--export=__heap_base",
    "-Clink-arg=--export=__tls_base",
    "-Clink-arg=--export=__tls_size",
    "-Clink-arg=--export=__tls_align",
    "-Clink-arg=--export=__wasm_init_tls",
    "-Clink-arg=--export=__wasm_call_ctors",
]

# A module linking C++ on wasi - a plugin's HarfBuzz, the app's on the web - is
# linked with the WASI sysroot's no-exceptions libc++, since the C++ is compiled
# with -fno-exceptions, and with its setjmp for FreeType. rustc links a wasm
# module itself, with rust-lld, so these are its flags rather than a C
# toolchain's.
WASI_LINK_FLAGS = [
    "-Clink-arg=-L$(execpath //bazel/tools:wasi-sysroot)/lib/wasm32-wasip1-threads/noeh",
    "-Clink-arg=-lc++abi",
    "-Clink-arg=$(execpath //bazel/tools:wasi-sysroot)/lib/wasm32-wasip1-threads/libsetjmp.a",
]

WASI_LINK_DATA = [Label("//bazel/tools:wasi-sysroot")]

_WASM = ["@platforms//cpu:wasm32"]

# An editor: the guest cdylib (wasm32 only; a plugin is all behind
# cfg(target_arch = "wasm32")), :module named after its manifest's entry point,
# :manifest, and its wasm :test.
def editor(name, module, visibility = ["//visibility:public"]):
    facts = cargo_wasm_facts()
    srcs = native.glob(["src/**/*.rs"])
    compile_data = native.glob(["src/**/*.wgsl", "manifest.json"], allow_empty = True)
    rust_shared_library(
        name = name + "_wasm",
        aliases = facts.aliases,
        compile_data = compile_data + WASI_LINK_DATA,
        crate_features = facts.features,
        crate_name = facts.crate,
        crate_root = facts.crate_root,
        deps = facts.deps,
        edition = facts.edition,
        proc_macro_deps = facts.proc_macro_deps,
        rustc_env = facts.env,
        rustc_flags = facts.rustc_flags + PLUGIN_EXPORTS + WASI_LINK_FLAGS,
        srcs = srcs,
        target_compatible_with = _WASM,
        version = facts.version,
        visibility = visibility,
    )
    wasi_module(
        name = "module",
        library = ":" + name + "_wasm",
        module = module,
        visibility = visibility,
    )
    native.alias(
        name = "manifest",
        actual = "manifest.json",
        visibility = visibility,
    )
    plugin_tests(
        srcs = srcs,
        compile_data = compile_data,
        exports = PLUGIN_EXPORTS,
    )

# A crate's tests compiled to wasm and run by plugin-test-runner, which gives
# the module a plugin's imports; an editor's, and block-editor-plugin's.
def plugin_tests(srcs, compile_data = [], exports = []):
    facts = cargo_wasm_facts()
    rust_binary(
        name = "test_module",
        aliases = facts.test.aliases,
        compile_data = compile_data + WASI_LINK_DATA,
        crate_features = facts.test.features,
        crate_name = facts.crate,
        crate_root = facts.crate_root,
        deps = facts.test.deps,
        edition = facts.edition,
        proc_macro_deps = facts.test.proc_macro_deps,
        rustc_env = facts.env,
        rustc_flags = facts.rustc_flags + exports + WASI_LINK_FLAGS + ["--test"],
        srcs = srcs,
        target_compatible_with = _WASM,
        version = facts.version,
    )
    wasi_test(
        name = "test",
        manifest = "Cargo.toml",
        module = ":test_module",
    )

# Compiled to wasm, a plugin's tests paint with the FreeType and HarfBuzz it
# ships, so the accepted paintings in snapshots/ stay put. The test runs here
# rather than on a worker, since it reads and writes those paintings: the guest
# finds them from CARGO_MANIFEST_DIR, the crate's real directory, which the
# test's own copy of Cargo.toml links to.
#
# The module is compiled for wasmtime once, in an action on a worker, rather
# than by every test process - about forty seconds a module - for the x86_64
# baseline, since the compile runs there and the test here.
def _wasi_test_impl(ctx):
    module = ctx.attr.module[0][DefaultInfo].files.to_list()
    module = [file for file in module if file.extension == "wasm"][0]
    precompiled = ctx.actions.declare_file(ctx.label.name + ".cwasm")
    ctx.actions.run(
        outputs = [precompiled],
        inputs = [module],
        executable = ctx.executable._precompiler,
        arguments = ["--precompile-to", "--target", "x86_64-unknown-linux-gnu", precompiled.path, module.path],
        mnemonic = "WasmPrecompile",
        progress_message = "Precompiling %{input}",
    )
    runner = ctx.executable._runner
    script = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(
        script,
        "\n".join([
            "#!/bin/sh",
            "set -eu",
            'manifest="$(readlink -f "{}")"'.format(ctx.file.manifest.short_path),
            'CARGO_MANIFEST_DIR="$(dirname "$manifest")"',
            "export CARGO_MANIFEST_DIR",
            'exec "{}" --precompiled "{}" "{}" "$@"'.format(runner.short_path, precompiled.short_path, module.short_path),
            "",
        ]),
        is_executable = True,
    )
    runfiles = ctx.runfiles([ctx.file.manifest, module, precompiled] + ctx.files.data)
    runfiles = runfiles.merge(ctx.attr._runner[DefaultInfo].default_runfiles)
    return [
        DefaultInfo(executable = script, runfiles = runfiles),
        RunEnvironmentInfo(environment = ctx.attr.env),
    ]

_wasi_test = rule(
    implementation = _wasi_test_impl,
    attrs = {
        "data": attr.label_list(allow_files = True),
        "env": attr.string_dict(),
        "manifest": attr.label(allow_single_file = True),
        "module": attr.label(cfg = wasi_guest_transition),
        "_precompiler": attr.label(default = "//crates/plugin-test-runner:plugin-test-runner-bin", executable = True, cfg = "exec"),
        "_runner": attr.label(default = "//crates/plugin-test-runner:plugin-test-runner-bin", executable = True, cfg = "target"),
    },
    test = True,
)

# What ./scripts/bazel run //:verify tells the plugin tests apart by: they run
# here (local) and accept paintings, and the rest run on a worker. They read
# the paintings in snapshots/, so a changed one runs them again.
def wasi_test(name, **kwargs):
    _wasi_test(
        name = name,
        data = ["//:snapshots"],
        size = "large",
        tags = ["local", "plugin"],
        target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        **kwargs
    )
