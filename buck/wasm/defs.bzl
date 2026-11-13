load(
    "@root//buck/cargo:defs.bzl",
    "cargo_example",
    "cargo_library",
    "cargo_library_sources",
    "cargo_manifest_sources",
    "cargo_test",
    "cargo_test_sources",
    "cargo_wasm_facts",
)
load("@root//buck/dev:test.bzl", "be_test")
load("@root//buck/platforms:cross.bzl", "per_cross_platform")
load("@root//buck/platforms:profile.bzl", "PROFILE_REFS", "RELEASE_SYMBOL_FLAGS", "RUST_LOCAL", "keep_profile", "opt_flags")
load("@root//buck/program:defs.bzl", "app_data", "app_types", "program")
load("@root//buck/wasm:libraries.bzl", "WasmLibrariesInfo")

# rustc links a cdylib with --no-entry, which strips the symbols a host needs to
# lay out a thread's storage and leaves nothing to run libc's constructors; the
# host does both itself, so a guest exports them.
_guest_exports = [
    "-Clink-arg=--export=__heap_base",
    "-Clink-arg=--export=__tls_base",
    "-Clink-arg=--export=__tls_size",
    "-Clink-arg=--export=__tls_align",
    "-Clink-arg=--export=__wasm_init_tls",
    "-Clink-arg=--export=__wasm_call_ctors",
]

# A program in the browser: the same exports, which wasm-bindgen needs to
# prepare the module for threads, and wasi-libc's setjmp and no-exceptions
# libc++ for the C and C++ it links.
_program_flags = _guest_exports + [
    "-Clink-arg=-L$(location root//third-party/wasi:sysroot)/lib/wasm32-wasip1-threads/noeh",
    "-Clink-arg=$(location root//third-party/wasi:sysroot)/lib/wasm32-wasip1-threads/libsetjmp.a",
]

# Every role a WebAssembly module has in the build, which is the platform it is
# built for (buck/platforms/BUCK) and how it is linked. Each has a transition
# and a module rule below, made from its entry, and nothing else names these
# platforms: wasm32_module, wasi_module (guest), wasi_program_module and
# wasi_html_module. `link_flags` are what every module of the role is linked with,
# which a crate's library takes as rustc flags in that configuration; a guest's
# also depend on its libraries (app_module).
#
# - wasm32, wasm32-unknown-unknown with no WASI: block types, games, the input
#   remaps and the browser's gpu shim, run in wasmi or by the page.
# - guest, wasm32-wasip1-threads with the `guest` constraint: an app and its
#   libraries (guides/app_libraries.md), linked by crates/be-wasm-link. The
#   same triple as the next two, told apart so that each gets its own wgpu.
# - program, the same triple: a program in the browser on beui's native core,
#   run through wasm-bindgen, such as be-host's web bundle.
# - html, the same with the `html` constraint: a program on beui's HTML core.
WASM_ROLES = {
    "guest": struct(link_flags = _guest_exports, platform = "root//buck/platforms:wasi_guest", setting = "root//buck/platforms:wasi_guest_setting"),
    "html": struct(link_flags = _program_flags, platform = "root//buck/platforms:wasi_html", setting = "root//buck/platforms:wasi_html_setting"),
    "program": struct(link_flags = _program_flags, platform = "root//buck/platforms:wasi", setting = "root//buck/platforms:wasi_setting"),
    "wasm32": struct(link_flags = [], platform = "root//buck/platforms:wasm32", setting = "root//buck/platforms:wasm32_setting"),
}

# WebAssembly modules, depended on from targets that are not WebAssembly: the
# app and the native tests only load a game or an app as a module, so the
# module is a dependency in another configuration, which a role's transition
# gives it. A game's test says `env = {"GAME_WASM": "$(location :module)"}`.
def _role_transition_impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
    return keep_profile(platform, refs.role[PlatformInfo], refs)

def _role_transition(role):
    return transition(impl = _role_transition_impl, refs = {"role": WASM_ROLES[role].platform} | PROFILE_REFS)

wasm32_transition = _role_transition("wasm32")

wasi_transition = _role_transition("guest")

wasi_program_transition = _role_transition("program")

wasi_html_transition = _role_transition("html")

# A library's flags for the role it is linked as, in that role's configuration
# alone: the library of a program in the browser is also built for the host,
# such as be-host's.
def role_link_flags(role):
    return select({
        "DEFAULT": [],
        WASM_ROLES[role].setting: WASM_ROLES[role].link_flags,
    })

# A cdylib's "shared" output is the .wasm; this names it as the module it is,
# which for an app is the entry point its manifest names. A release build runs
# wasm-opt over it: after rustc, -O2 takes a further tenth off an app's code
# and a third off a game, where -Oz takes a little more at twice the time. -g
# keeps the name section, which is what a guest's backtrace is read with.
# --one-caller-inline-max-function-size stops it inlining every function with
# one caller however large: it merged the gallery's components into one 925 KB
# function and put back together the relocation function the linker splits,
# past what Windows on ARM64 compiles (guides/app_libraries.md, Function size).
def _wasm_module_impl(ctx: AnalysisContext) -> list[Provider]:
    shared = ctx.attrs.library[DefaultInfo].sub_targets["shared"][DefaultInfo].default_outputs[0]
    name = (ctx.attrs.module or ctx.attrs.name) + ".wasm"
    closure = []
    for library in ctx.attrs.libraries:
        for module in library[WasmLibrariesInfo].closure + [library[DefaultInfo].default_outputs[0]]:
            if module not in closure:
                closure.append(module)
    libraries = WasmLibrariesInfo(closure = closure)
    if not ctx.attrs.optimize:
        return [DefaultInfo(default_output = ctx.actions.copy_file(name, shared)), libraries]
    module = ctx.actions.declare_output(name)
    ctx.actions.run(
        cmd_args(
            ctx.attrs._wasm_opt[RunInfo],
            "-O2",
            "--one-caller-inline-max-function-size=1000",
            "-g",
            "--detect-features",
            shared,
            "-o",
            module.as_output(),
        ),
        category = "wasm_opt",
    )
    return [DefaultInfo(default_output = module), libraries]

def _module_rule(cfg):
    return rule(
        attrs = {
            # The libraries it is linked against (wasm_library), whose
            # closure it gives as WasmLibrariesInfo.
            "libraries": attrs.list(attrs.dep(providers = [WasmLibrariesInfo]), default = []),
            "library": attrs.transition_dep(cfg = cfg),
            "module": attrs.option(attrs.string(), default = None),
            "optimize": attrs.default_only(
                attrs.bool(
                    default = select({
                        "DEFAULT": False,
                        "root//buck/constraints:release": True,
                    })
                )
            ),
            "_wasm_opt": attrs.exec_dep(default = "root//buck/tools:wasm-opt"),
        },
        impl = _wasm_module_impl,
    )

wasm32_module = _module_rule(wasm32_transition)

wasi_module = _module_rule(wasi_transition)

wasi_program_module = _module_rule(wasi_program_transition)

wasi_html_module = _module_rule(wasi_html_transition)

# A crate that is a wasm32 module too: the package's library, for the host,
# and the same crate with extra_features as :wasm, built only for wasm32, so
# asking for it on the host is a configuration error rather than a link
# failure, and linked as :module, whatever the thing asking for it is built
# for. module names the .wasm, which is the crate's own name without it.
def wasm32_crate(module = None, extra_features = []):
    cargo_library()
    cargo_library(
        name = "wasm",
        extra_features = extra_features,
        preferred_linkage = "shared",
        target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"],
    )
    wasm32_module(
        name = "module",
        library = ":wasm",
        module = module,
    )

# An app's block types (guides/type_modules.md): the content as a plain
# library, which the editor and its tests depend on, its tests unless test =
# False, and :module, the type module the app runs it in, which only the
# crate's `module` feature exports.
def type_module(module, test = True):
    wasm32_crate(extra_features = ["module"], module = module)
    if test:
        cargo_test()

# The rules of one game (guides/adding_a_game.md), twice: as :module, the only
# form the app ever loads, and natively, so that the tests drive :module,
# GAME_WASM, through the host.
def game_module():
    wasm32_crate()
    cargo_test(env = {"GAME_WASM": "$(location :module)"})

# The library of a program in the browser, linked as role ("program" or
# "html") in that role's configuration (role_link_flags), and built as it is
# everywhere else.
def web_library(role, rustc_flags = [], **kwargs):
    if role not in ["program", "html"]:
        fail("a web library is a program or html, not " + role)
    cargo_library(rustc_flags = role_link_flags(role) + rustc_flags, **kwargs)

# What every program in the browser serves beside its module: be-host's shims
# for WASI and threads, which the module's JavaScript imports.
_WEB_SHIMS = [
    "//crates/be-host:wasi.js",
    "//crates/be-host:threads.js",
    "//crates/be-host:thread.js",
    "//crates/be-host:dylink.js",
    "//crates/be-host:env.js",
]

# A program in the browser (buck/program/defs.bzl): the package's web_library
# linked as role into :web-module, named module.wasm, through wasm-bindgen,
# with files and the shims. serve is the address `./scripts/buck run` serves it
# on with Caddy, which run_with replaces for a program that serves itself
# another way, and either can be left out for one nothing runs.
def web_program(name, role, module, files = [], serve = None, **kwargs):
    module_rule = {"html": wasi_html_module, "program": wasi_program_module}[role]
    module_rule(
        name = "web-module",
        library = ":" + cargo_wasm_facts().name,
        module = module,
    )
    if serve != None:
        kwargs["run_with"] = [
            "sh",
            "$(location //buck/program:serve-bundle.sh)",
            "$(location //buck/tools:caddy)",
            "$(location //buck/program:bundle.Caddyfile)",
            serve,
        ]
    program(name = name, bindgen = [":web-module"], files = files + _WEB_SHIMS, manifests = [], modules = [], wasm_bindgen = "//buck/cargo:wasm-bindgen", **kwargs)

# The libraries every editor is linked against unless it names its own
# (guides/app_libraries.md): base, the root that holds std, wasi-libc, wgpu and
# be-sdk, and beui above it.
BASE_LIBRARY = "root//crates/be-sdk-library-base:module"

DEFAULT_LIBRARIES = [BASE_LIBRARY, "root//crates/be-sdk-library-beui:module"]

# What makes a module the root of every closure: wasi-libc's crt1-reactor.o,
# whose _initialize sets up libc's main thread and runs the constructors. The
# root and a standalone app are each a WASI reactor linked at fixed
# addresses, which the host instantiates as it is.
_reactor = [
    "-Clink-arg=$(location root//third-party/wasi:sysroot)/lib/wasm32-wasip1-threads/crt1-reactor.o",
    "-Clink-arg=--export=_initialize",
]

def _needs(libraries):
    return ["--be3-needs=$(location {})".format(library) for library in libraries]

# A library apps are linked against (guides/app_libraries.md): the package's
# crate linked whole into a module of its own that exports everything, which
# the host links each app that names it to when it loads. libraries names
# every library under it, the whole closure; with none it is the root, linked
# at fixed addresses with std and wasi-libc, and with some it is
# position-independent, leaves out everything they link and reaches their
# thread-locals through GOT.tls. A package whose crate other crates depend on,
# such as crates/be-tool-tls-diamond-base, has a library.rs beside its Cargo.toml that only
# names the crate, and that is what is linked, so the module holds the very rlib
# its dependents compile against.
def wasm_library(module, libraries = [], rustc_flags = []):
    facts = cargo_wasm_facts()
    linker_flags = _needs(libraries) + ["--be3-library=" + module]
    rustc_flags = _guest_exports + rustc_flags + ([] if libraries else _reactor) + RELEASE_SYMBOL_FLAGS
    if native.glob(["library.rs"]):
        cargo_library(rustc_flags = rustc_flags)
        native.rust_library(
            name = "wasm",
            crate = facts.crate + "_library",
            crate_root = "library.rs",
            deps = [":" + facts.name],
            edition = facts.edition,
            linker_flags = linker_flags,
            preferred_linkage = "shared",
            rustc_flags = rustc_flags,
            srcs = ["library.rs"],
            target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"] + facts.compatible,
            **RUST_LOCAL,
        )
    else:
        native.rust_library(
            name = "wasm",
            crate = facts.crate,
            crate_root = facts.crate_root,
            deps = facts.deps,
            edition = facts.edition,
            env = facts.env,
            features = facts.features,
            linker_flags = linker_flags,
            mapped_srcs = cargo_manifest_sources(),
            preferred_linkage = "shared",
            rustc_flags = rustc_flags,
            srcs = cargo_library_sources(),
            target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"] + facts.compatible,
            **RUST_LOCAL,
        )
    wasi_module(
        name = "module",
        libraries = libraries,
        library = ":wasm",
        module = module,
    )

# A package's crate linked against libraries, every one in its closure, and
# exporting only what an app does: an app that is not an editor, such as
# crates/be-tool-tls-diamond-app.
def linked_app(module, libraries, rustc_flags = []):
    facts = cargo_wasm_facts()
    native.rust_library(
        name = "wasm",
        crate = facts.crate,
        crate_root = facts.crate_root,
        deps = facts.deps,
        edition = facts.edition,
        env = facts.env,
        features = facts.features,
        linker_flags = _needs(libraries),
        mapped_srcs = cargo_manifest_sources(),
        preferred_linkage = "shared",
        rustc_flags = _guest_exports + rustc_flags + RELEASE_SYMBOL_FLAGS,
        srcs = cargo_library_sources(),
        target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"] + facts.compatible,
        **RUST_LOCAL,
    )
    wasi_module(
        name = "module",
        libraries = libraries,
        library = ":wasm",
        module = module,
    )

# The package's app linked as :<name>, its module named module.wasm: what
# app() makes :module with, and a second build of the same app linked
# another way, such as triangle's :standalone. libraries = None links it
# against DEFAULT_LIBRARIES; a list names every library in its closure; [] links
# every crate it uses into it, a standalone app (guides/app_libraries.md, "The
# raw app interface"), a WASI reactor the host instantiates as it is.
def app_module(name, module, libraries = None, extra_deps = [], visibility = None):
    kwargs = {"visibility": visibility} if visibility != None else {}
    facts = cargo_wasm_facts()
    if libraries == None:
        libraries = DEFAULT_LIBRARIES
    linker_flags = _needs(libraries)
    rustc_flags = [] if libraries else _reactor
    library = "wasm" if name == "module" else name + "-wasm"
    native.rust_library(
        name = library,
        crate = facts.crate,
        crate_root = facts.crate_root,
        deps = facts.deps + extra_deps,
        edition = facts.edition,
        env = facts.env,
        features = facts.features,
        linker_flags = linker_flags,
        mapped_srcs = cargo_manifest_sources(),
        preferred_linkage = "shared",
        rustc_flags = _guest_exports + rustc_flags + opt_flags("app") + RELEASE_SYMBOL_FLAGS,
        srcs = cargo_library_sources() + ["manifest.json"],
        target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"] + facts.compatible,
        **(kwargs | RUST_LOCAL),
    )
    wasi_module(name = name, libraries = libraries, library = ":" + library, module = module, **kwargs)

# An app: the guest cdylib, :wasm (wasm32 only; an app is all behind
# cfg(target_arch = "wasm32")), :module named after its manifest's entry point,
# :manifest, and its wasm :test. test_env is extra environment for compiling the
# tests, which is how a test names a module it loads with include_bytes!.
# extra_deps are what cargo cannot name, such as a C library the app links.
# An app is linked against DEFAULT_LIBRARIES, or as libraries says
# (app_module above). types is the app's type module
# (a wasm32_module, such as checklist's //crates/be-type-checklist:module),
# which :types names for the app to stage beside :module.
#
# The app crate itself, its tests, its workspace dependencies and its
# third-party ones are each compiled at their own opt level (OPT_LEVELS in
# buck/platforms/profile.bzl).
#
# An app with examples/ is built for the host too (PLANS in
# crates/be-tool-buck/src/plan.rs plans it there), as the library named after its package, and each
# examples/NAME.rs is :example-NAME on the host, such as a benchmark that
# drives the editor through be-sdk-test.
def app(module, visibility = None, test_env = {}, data = {}, extra_deps = [], types = None, libraries = None):
    kwargs = {"visibility": visibility} if visibility != None else {}
    app_module(
        name = "module",
        extra_deps = extra_deps,
        libraries = libraries,
        module = module,
        visibility = visibility,
    )
    native.export_file(name = "manifest", src = "manifest.json", **kwargs)
    app_data(name = "data", files = data, **kwargs)
    app_types(name = "types", module = types, **kwargs)
    app_tests(
        env = test_env,
        exports = _guest_exports,
        extra_deps = extra_deps,
    )
    examples = native.glob(["examples/*.rs"])
    if examples:
        cargo_library(extra_srcs = ["manifest.json"])
        for root in examples:
            cargo_example(root.removeprefix("examples/").removesuffix(".rs"))

# A crate's tests compiled to wasm and run by app-test-runner, which gives
# the module an app's imports; an editor's, be-sdk's and
# be-sdk-beui's.
# The test crate itself is compiled at app_test's opt level, and its
# dependencies as the app's are.
def app_tests(exports = [], env = {}, extra_deps = []):
    facts = cargo_wasm_facts()
    native.rust_binary(
        name = "test-module",
        crate = facts.crate,
        crate_root = facts.crate_root,
        deps = facts.test_deps + extra_deps,
        edition = facts.edition,
        env = facts.env | env,
        features = facts.test_features,
        mapped_srcs = cargo_manifest_sources(),
        rustc_flags = exports + ["--test"] + opt_flags("app_test"),
        srcs = cargo_test_sources() + native.glob(["manifest.json"]),
        target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"] + facts.compatible,
        **RUST_LOCAL,
    )
    wasm_test_harness(
        name = "test-harness",
        module = ":test-module",
        precompile = per_cross_platform(True, lambda _: False),
        runner = "//crates/be-sdk-test-runner:bin",
    )
    be_test(
        name = "test",
        env = {
            "LD_LIBRARY_PATH": "$(location //buck/sysroot:amd64-test)/usr/lib/x86_64-linux-gnu",
            "VK_ICD_FILENAMES": "$(location //buck/sysroot:lavapipe_icd.json)",
        },
        harness = ":test-harness",
        labels = ["app"],
        manifest = "Cargo.toml",
        paintings = native.glob(["snapshots/*.paint"]),
        paints = True,
        test_type = "rust",
    )

# Compiled to wasm, an app's tests paint with the FreeType and HarfBuzz it
# ships, so the accepted paintings in the crate's snapshots/ stay put. The guest
# finds them from CARGO_MANIFEST_DIR, taken from the crate's real Cargo.toml
# rather than a staged copy, and app-test-runner opens that directory, and
# USED_PAINTINGS' and CHANGED_PAINTINGS', to it and nothing else.
#
# The module is compiled for wasmtime once, in an action, rather than by every
# test process - about forty seconds a module. It is compiled for the x86_64
# baseline, since the compile runs on a worker and the test here, and only for
# the host: elsewhere the runner compiles it when the test runs. --quick
# compiles it without Cranelift's optimizations, which took a quarter off the
# compile and left the tests' running time as it was; Cranelift's single-pass
# register allocator, which would take off more, ran a deep paint out of stack.
def _precompiled(ctx: AnalysisContext, module: Artifact) -> cmd_args:
    if not ctx.attrs.precompile:
        return cmd_args(module)
    runner = ctx.attrs.runner[RunInfo]
    artifact = ctx.actions.declare_output(module.basename.removesuffix(".wasm") + ".cwasm")
    ctx.actions.run(
        cmd_args(runner, "--precompile-to", "--target", "x86_64-unknown-linux-gnu", "--quick", artifact.as_output(), module),
        category = "wasm_precompile",
        identifier = module.basename,
    )
    return cmd_args("--precompiled", artifact, module)

# The app test's command: app-test-runner and the module, which draws through
# the sysroot's lavapipe, as the renderer's tests do (the env app_tests gives
# its be_test).
def _wasm_test_harness_impl(ctx: AnalysisContext) -> list[Provider]:
    module = ctx.attrs.module[DefaultInfo].default_outputs[0]
    return [
        DefaultInfo(default_output = module),
        RunInfo(args = cmd_args(ctx.attrs.runner[RunInfo], _precompiled(ctx, module))),
    ]

wasm_test_harness = rule(
    attrs = {
        "module": attrs.transition_dep(cfg = wasi_transition),
        "precompile": attrs.bool(default = True),
        "runner": attrs.dep(providers = [RunInfo]),
    },
    impl = _wasm_test_harness_impl,
)
