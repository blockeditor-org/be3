# buck2

buck2 builds and tests the workspace, and every action it runs, runs on
BuildBuddy. This guide says what it covers, how to run it, why it is set up the
way it is, and what is still cargo's.

`./scripts/verify` runs the tests through it. cargo is still what `./scripts/check`,
the lint pass, `./scripts/build` and `./scripts/run` use, and what builds for
every platform but Linux on x86_64; the last section says why each of those is
still cargo's.

## What buck2 builds

- Every third-party crate the workspace depends on, on `x86_64-unknown-linux-gnu`:
  around nine hundred of them, including the ones that compile C or assembly -
  sqlite, zstd, freetype, harfbuzz, tree-sitter, ring, wasmtime.
- Every first-party crate and its tests: the `be-*` stack, `block*`, `beui`,
  `reactive`, `text-editor-core`, the games' api and host, the tools, and
  `block-app` itself, with its terminal and without its embedded browser.
- libghostty-vt, the Zig library behind the terminal, built on a worker by the
  same script cargo's build uses.
- The three game modules, as WebAssembly, and the native tests that drive them
  through the host.
- All thirty-three editors, as the wasm plugins the app loads, and their tests,
  compiled to wasm and run through the same host `block-app` runs a plugin in.

`./scripts/buck test //crates/...` is 71 test targets.

## Running it

```
export BUILDBUDDY_API_KEY=<key>
./scripts/buck build //crates/...
./scripts/buck test //crates/...
./scripts/buck test //crates/editors/checklist:test
./scripts/buckify
```

`./scripts/buck` is buck2 with two checks in front: that `BUILDBUDDY_API_KEY` is
set, since there is nowhere else to build, and that no `.buckconfig.local` an
older copy of it wrote is left behind. It also lets a test run put its tests on
the workers (below). Calling `buck2` directly works too, once the key is in the
environment.

**Restart the daemon when the key changes**, with `buck2 killall`. `.buckconfig`
names the variable rather than holding the key, and buck2 expands it from the
environment of the daemon, which is the one it was started with.

`./scripts/buckify` regenerates `third-party/rust/BUCK` and
`buck/cargo/crates.bzl`. Run it after changing any `Cargo.toml` or
`Cargo.lock`; CI fails if either disagrees with the manifests. Nothing else
needs touching for a new dependency: a crate's `BUCK` file takes its
dependencies from `crates.bzl` (below).

The tools are installed by `./scripts/internal/install-buck2.sh` and
`install-starlark-fmt.sh`, which are downloads. reindeer is not installed at
all: `./scripts/buckify` runs it on a worker (below).

### Generated from Cargo.toml

Two files are generated, and both are checked in:

- `third-party/rust/BUCK`, one rule per third-party crate, which reindeer
  writes.
- `buck/cargo/crates.bzl`, what each workspace crate's `Cargo.toml` says - its
  dependencies and features on each platform, its edition and its targets -
  which `buck/cargo/generate.py` writes. The `BUCK` file beside a crate turns
  that into rules with `cargo_library()`, `cargo_test()` and `cargo_binary()`
  from `buck/cargo/defs.bzl`, and the plugin macros in `buck/wasm/defs.bzl`
  read it the same way.

A crate's dependencies and features come from cargo's own plan for the build
buck2 stands in for, `cargo test --unit-graph` or `cargo build --unit-graph`,
rather than from `cargo metadata`. The difference matters: cargo unifies
features across one invocation, so `beui` has `window` in a build of the app
and only `render` in a build of the plugins, and `cargo metadata` would give it
`window` everywhere. There are three plans - the host, the plugins for
`wasm32-wasip1-threads`, the games for `wasm32-unknown-unknown` - and
`buck/cargo/buckify.bxl` says exactly how each is asked for. Where they differ
for a crate, the macro writes the `select()`.

`buck/cargo/buckify.bxl` is what `./scripts/buckify` runs. It builds reindeer
on a worker - `cargo install` of the pinned commit, with the nightly reindeer's
own rust-toolchain asks for, both in `buck/cargo/BUCK` - and then, in one
action on a worker, runs `reindeer buckify --stdout` against
rust-toolchain.toml's cargo and the three plans against the nightly's, and
prints the directory the two files are in. The script copies them into the
repository. Building reindeer takes about nine minutes, once; regenerating
takes about a minute, most of it cargo downloading the crates, and only when
something it reads has changed.

What it reads is the root `Cargo.toml`, `Cargo.lock`, `reindeer.toml`, the
fixups, `generate.py`, and every `Cargo.toml` under `crates/`: those are the
action's real inputs. cargo also looks at the layout of each crate - whether
there is a `src/lib.rs`, a `build.rs`, an `examples/` directory - but never
reads a source file, so every other file under `crates/` is passed as a path
only and recreated empty on the worker. An edit to a source file leaves the
action's key alone; adding a file, a dependency or a crate is what runs it
again. With nothing changed, `./scripts/buckify` takes about a second on a live
daemon.

It is a BXL script rather than a rule because the manifests belong to
seventy-odd packages, and a rule can only take the files of its own package.

## BuildBuddy

Every action runs on one of BuildBuddy's workers, and BuildBuddy keeps the
result, so a build only ever does the work nobody has done before:

```
./scripts/buck build //crates/...    # BuildBuddy empty
Commands: 4699 (cached: 0, remote: 4699, local: 0)
real    4m07s

./scripts/buck test //crates/...     # after buck2 clean
Cache hits: 100%
Commands: 3610 (cached: 3610, remote: 0, local: 0)
Tests finished: Pass 68. Fail 0.
real    0m20s
```

Four minutes is the whole workspace from nothing. The same machine takes nine
to build `beui` and one plugin by itself, and cargo's own test run takes
fourteen from an empty `target/`.

What this machine downloads is what it asked for and what a local test runs;
the intermediate rlibs stay in BuildBuddy's CAS and never arrive. Nothing is
uploaded from here: BuildBuddy writes the result of every action a worker ran,
and a machine never vouches for an output it compiled itself.

### What a worker is

A worker is a container with nothing of this project's in it: Ubuntu 24.04's
`buildpack-deps`, pinned by digest in `buck/tools/defs.bzl`. It brings the parts
of a build that are the distribution's rather than the project's - glibc and
its headers, libstdc++, the gcc install clang takes them from, and a Python new
enough for the prelude.

Everything else comes with the build, as inputs of the actions that use it:

- `buck/tools/BUCK` downloads `rust-toolchain.toml`'s Rust from
  static.rust-lang.org - rustc, clippy and the three standard libraries - and
  Ubuntu 24.04's clang-20, lld and llvm-ar packages from Launchpad, which keeps
  every version it ever published.
- `third-party/system/BUCK` does the same for ALSA, which `rodio` links and the
  container does not have.
- `crates/ghostty-vt/BUCK` runs `scripts/internal/build-ghostty-vt.sh` on a
  worker, which downloads Zig and Ghostty itself. It is the one action that
  needs the network, and its inputs pin everything it downloads.

Every download is pinned by size as well as hash. With both, buck2 knows the
file's digest without asking the server, so a build whose workers already have
it never contacts the host that serves it; with only the hash it sends a HEAD
request per download on every new daemon, and fails the build when a host is
slow to answer.

Because the tools are artifacts rather than names, they are part of every
action's key: a different rustc or clang is a different action, never a cache
hit on another compiler's output. The container is part of the key too, through
the platform properties every action carries.

### Tests

Most tests run on the workers as well. Three kinds stay on the machine that
asked, because a worker cannot do what they do:

- **The plugin tests** read the accepted paintings out of `snapshots/` in the
  working tree and write the ones that changed back into it, and may open a
  graphics adapter. They are labelled `plugin`, which is how
  `./scripts/verify --tests` and `--plugin-tests` tell them apart.
- **`beui`'s renderer tests** draw through a real graphics adapter, and the
  container has none. Its `rust_test` says `remote_execution = "disabled"`.
- **`block-plugin-api`'s `every_editor_manifest_parses`** walks
  `crates/editors` in the working tree, and the editors are packages of their
  own rather than its inputs. Same attribute.

buck2 keeps every test local unless it is told otherwise, so `./scripts/buck`
adds `--unstable-allow-compatible-tests-on-re` to a test run. "Compatible" is
what separates them: a Rust test runs from the project root with
project-relative paths, which is what a worker can give it, and the plugin
tests' rule asks for absolute ones.

A local test runs a binary a worker linked, against the worker's glibc. That is
Ubuntu 24.04's, so a machine on an older one cannot run them.

Arguments after `--` go to buck2's test executor rather than the test.
`--env NAME=VALUE` sets a variable for the tests - `UPDATE_SNAPSHOTS=1` is how
`./scripts/verify` accepts paintings - and `--test-arg` passes one through to
the test binary, which is how to run a single test:

```
./scripts/buck test //crates/editors/checklist:test -- --env UPDATE_SNAPSHOTS=1
./scripts/buck test //crates/editors/checklist:test -- --test-arg some_test_name
```

buck2's test runner starts one process per test binary and runs its tests on
threads, the way `cargo test` does, where nextest gave every test a process of
its own. A test that installs process-wide state has to take turns with the
others that do; `block-app`'s `be` tests hold a lock for that.

### When the key is wrong

Three things about the connection are not guessable, and none of them
announces itself. The address is a bare `remote.buildbuddy.io:443`: buck2
parses it itself and rejects both `grpcs://` and `https://` with `Invalid
URI`. `http_headers` separates name from value with a colon, not an equals
sign. And the instance name is empty.

buck2 treats a cache it cannot use as a cache that is empty, so a wrong key
shows up as a build with no hits rather than as an error. BuildBuddy's
rejection is in the event log rather than on screen:

```
buck2 log show | grep -i 'invalid api key'
```

### Build metadata

BuildBuddy's build metadata - the repository, branch, commit and role that
group invocations and drive its GitHub commit statuses - is not available here.
BuildBuddy reads it from the Build Event Stream, which is Bazel's, and buck2
does not implement it; nothing in the pinned release speaks it, and buck2's own
`--client-metadata` goes only to its event log. What BuildBuddy does see is the
remote execution traffic itself: the actions, their inputs and outputs, and
what they cost.

## WebAssembly

The games build for `wasm32-unknown-unknown` and the plugins will build for
`wasm32-wasip1-threads`. Both are target platforms in `buck/platforms/BUCK`,
and asking for one is:

```
./scripts/buck build //crates/tabletop_games/rules/tic_tac_toe:tic_tac_toe_wasm \
    --target-platforms=//buck/platforms:wasm32
```

Three things make that work, and all three are worth knowing about before
adding the plugins to it.

**The toolchain switches on what is being built for.** `buck/toolchains/BUCK`
selects the target triple and the rustc flags off the target platform, so wasm
gets Cargo.toml's plugin profile rather than its dev one. The C toolchain
switches too: rustc emits lld's own flags for a wasm link and hands them to
whatever the cxx toolchain calls a linker, which for the host is a clang driver
that has never heard of them, so wasm gets `rust-lld` from the same rustc
instead. `buck/tools/wasm-ld` drops `-fuse-ld=lld` on the way through, because
`prelude//os_lookup` has no case for WebAssembly and falls back to linux - its
own FIXME says so - and a linux link is what the prelude thinks it is building.

**The C dependencies compile to wasm.** `third-party/wasi:sysroot` is the WASI
sysroot as an `http_archive`, checked against the same hash
`scripts/internal/common.sh` pins for the cargo build, and `buck/toolchains/BUCK`
has a cxx toolchain pointed at it with the flags the cargo build passes - the
setjmp lowering FreeType needs, the `-pthread` that marks the objects as using
atomics, and the rest. Rust needs none of it: rustc's own wasip1 standard
library is self-contained, and a pure-Rust module links without the sysroot at
all. `beui` builds for `wasm32-wasip1-threads` through this, freetype and
harfbuzz included.

The wasm C compiler is the host one aimed elsewhere: the same downloaded
clang-20, with `--target=wasm32-wasip1-threads` in the tool itself rather than
in the toolchain's flags, because the prelude's cxx toolchain has flags for C
and for C++ and none for assembly. FreeType's setjmp lowering is why it is
clang 20: it needs 19 or newer, which is also what the cargo build picks.

**reindeer resolves the third-party crates once per platform**, and
`reindeer.toml` names all four. The platform names are not free: a buck2
configuration is mapped to one of them, so `wasm32` is `os:none` + `cpu:wasm32`,
and `wasi` and `wasi-guest` are `os:wasi` with the same cpu. The root `PACKAGE`
file is what does the mapping, through the prelude's
`set_reindeer_platforms`.

What that costs is feature fixups. reindeer resolves one feature set per
platform across the whole workspace, so a feature one crate wants natively
arrives everywhere: `uuid`'s `v4` and `rand`'s `os_rng` both reached
`wasm32-unknown-unknown`, where there is no operating system to ask for entropy
and the crates say so with a `compile_error`. The fixups under
`third-party/rust/fixups` take them back off for that platform.

**`wasi` and `wasi-guest` are the same triple.** `block-app`'s web bundle and a
plugin are both `wasm32-wasip1-threads`, and no cfg tells them apart, but they
want different `wgpu`: the app draws to the page and links a real browser
backend, and a plugin is a guest that reaches the host's device through the gpu
abi with only wgpu's `custom` backend. The two cannot be merged. `custom` keeps
its handles behind trait objects that are neither `Send` nor `Sync`, and
`fragile-send-sync-non-atomic-wasm` - which the app has, because this target's
cfg carries no `"atomics"` - makes wgpu assert that they are both.

cargo keeps them apart by resolving features once per call and making two calls.
reindeer resolves once per platform, so the platforms are what differ:

- `buck/constraints/BUCK` has a `wasm_role` setting with a `guest` value.
- `buck/platforms:wasi_guest` is `buck/platforms:wasi` plus that value, and
  `buck/wasm/defs.bzl` transitions a plugin's module and tests to it.
- `[platform.wasi-guest]` in `reindeer.toml` is the same triple with an extra
  `plugin_guest` cfg that nothing but the fixups reads. Cargo resolves both
  platforms identically; the fixups on `wgpu` and `egui-wgpu` are keyed on
  `cfg(plugin_guest)` and are what then give each one its own feature set.

Everything else follows: `wgpu-types` loses `fragile-send-sync-non-atomic-wasm`
on the guest and `wgpu-core` and `wgpu-hal` appear only on the app's, because
reindeer resolves the forwarding for each platform in turn. Measured against
cargo, those three crates are the whole of the difference between the two
builds - everything else the app wants is a superset the guest compiles with
anyway.

**A native target can depend on a wasm one.** `buck/wasm/defs.bzl` has a rule
that transitions its dependency to a wasm platform, so a test that is built for
the host can name a module that is not:

```
env = {"GAME_WASM": "$(location :module)"}
```

That replaces the build script under cargo, which shells out to a second cargo
build for wasm32 and prints the path it wrote to. The plugin tests will want the
same rule.

## The compiler is a stable one, and the build says so

`nightly_features = False` in `buck/toolchains/BUCK`, which is not the prelude's
default. Left on, the rules pass `RUSTC_BOOTSTRAP=1` to every compile so they
can use `-Z` flags - `-Zno-codegen` for pipelined builds, `-Zremap-cwd-prefix`
for paths. That variable does not only unlock flags. It makes a stable rustc
behave like a nightly one everywhere the difference is observable, and the one
that matters here is `cfg(target_feature)`: a nightly reports unstable target
features and a stable one does not.

`wasm32-wasip1-threads` has `atomics`, and `atomics` is unstable to name. So
under `RUSTC_BOOTSTRAP=1` a crate asking `not(target_feature = "atomics")` got
the opposite answer from the one cargo gives it, while the build script that
asked the same question through `CARGO_CFG_TARGET_FEATURE` still got cargo's.
`wgpu-types` is such a crate - that cfg is what decides whether its handles are
`Send` and `Sync` - and `wgpu-core` asserted the answer the build script had
given it. The symptom was `dyn DynInstance` not being `Sync` in a build whose
features matched cargo's exactly. A build script probing for `#![feature(...)]`
would go the same way, and silently.

What it costs is one layer of pipelining: a dependent that has to generate code
waits for the rlib rather than for a `-Zno-codegen` hollow one. The unstable
paths this project never used go with it - doctests, the `[expand]`,
`[doc-coverage]` and profiling subtargets - and the cwd is still remapped, by
the action wrapper rather than by `-Zremap-cwd-prefix`.

## Clippy

The rust toolchain carries `clippy_driver`, so every rust target has a
`clippy.txt` subtarget:

```
./scripts/buck build '//crates/reactive:reactive[clippy.txt]'
```

The lint levels are on the toolchain in `buck/toolchains/BUCK` -
`deny_lints = ["warnings"]` and the one allow that Cargo.toml's
`[workspace.lints]` sets - and `clippy.toml` at the root is clippy's own
configuration, which is a separate thing and is empty. It exists because clippy
under buck2 is handed a configuration directory rather than left to search for
one, and a directory without a `clippy.toml` in it is an error rather than a
default.

One thing to know before building anything on this: **the subtarget writes the
diagnostics to a file and the build still succeeds**. A lint gate has to build
the subtarget for every target and then check that each output is empty; a
green `buck2 build` says nothing about whether clippy was happy.

`./scripts/verify` still lints with cargo. buck2 builds `block-app` without its
embedded browser, so switching would quietly stop linting that half of it; and
a gate on the subtarget has the empty-file problem above to solve first.

## How the build is laid out

- `.buckconfig` names the cells, the execution platform and BuildBuddy. The
  prelude is the one bundled in the buck2 binary, so there is no submodule to
  check out and no prelude version to keep in step with the binary: the pinned
  buck2 release pins both.
- `buck/tools/BUCK` downloads the compilers, and `defs.bzl` beside it holds the
  rules that lay them out and the container a worker runs.
- `buck/toolchains/BUCK` is the toolchain, built on `buck/tools`; `defs.bzl`
  beside it holds the small rules that take an artifact where the prelude's own
  toolchains take a name, and the wrapper that lets links run on a worker and
  puts a binary's shared libraries beside it. rustc gets `-Copt-level=0`, which
  is cargo's dev profile and also what makes the prelude tell a build script
  what `OPT_LEVEL` it is building for.
- `buck/platforms/BUCK` is the execution platform: the prelude's, pointed at
  BuildBuddy's workers.
- `third-party/rust/BUCK` is generated by reindeer from the workspace manifests.
  It is checked in and nothing but `./scripts/buckify` should edit it. Crates
  are downloaded from crates.io at build time rather than vendored, so the
  repository carries the rules and not nine hundred crates of source.
- `third-party/rust/fixups/<crate>/fixups.toml` is what reindeer needs told
  about a crate it cannot work out on its own. Every crate with a build script
  needs one, if only to say `buildscript.run = true`.
- `third-party/system/BUCK` is for libraries the system provides rather than
  the build: the C++ runtime a Rust binary linking harfbuzz needs, and ALSA.
- `crates/<crate>/BUCK` is written by hand, one per crate.

## Adding a crate

Add it to the workspace in the root `Cargo.toml`, run `./scripts/buckify`, and
write a `BUCK` file beside its `Cargo.toml` naming the targets it has:

```
load("@root//buck/cargo:defs.bzl", "cargo_binary", "cargo_library", "cargo_test")

cargo_library()

cargo_test()

cargo_binary()
```

Everything a rule needs that is in `Cargo.toml` comes from `crates.bzl`: the
dependencies (third-party ones as `//third-party/rust:<package>`, workspace ones
as `//crates/<dir>:<package>`), the features, the edition, the crate name and
root, and cargo's package environment. What is left for the `BUCK` file is what
`Cargo.toml` cannot say, passed as arguments:

- `extra_deps` for something that is not a crate, like `ghostty-vt`'s Zig
  archive.
- `env` for a variable beyond cargo's, like a game's `GAME_WASM`.
- `srcs` when the crate reads files at compile time that are not Rust - a
  `.wgsl` shader, a font, an `examples/` file a test includes. The default is
  `src/**/*.rs`.
- Any other rule attribute as it is, like a test's `remote_execution =
  "disabled"`.

A few conventions the macros follow:

- The library is named after the package, and its tests are `:test`.
- A binary named like its package is `<name>-bin`, since the library has the
  name; any other binary keeps its own name, and a crate with several names the
  one with `bin = "..."`.
- A proc macro's tests get `--extern proc_macro` without being asked.
- An editor is one `editor(name, module)` call instead, from
  `buck/wasm/defs.bzl`.

## The plugins

Every editor is one `editor()` call in `crates/editors/<name>/BUCK`, and the
macro in `buck/wasm/defs.bzl` makes three targets out of it:

- `<name>_wasm`, the guest, a cdylib for `wasm32-wasip1-threads`. There is no
  native build of an editor, so it is compatible with `wasm32` alone.
- `module`, that cdylib under the name the manifest's `entry_point` gives it.
- `test`, which is the editor's tests compiled to wasm and run through
  `plugin-test-runner`, the same host `block-app` runs a plugin in.

A plugin paints with the FreeType and HarfBuzz it was compiled against, so its
tests only mean anything as a wasm guest: run natively they paint with whatever
those libraries happen to be on the machine and the accepted paintings never
settle. Under cargo that is `CARGO_TARGET_WASM32_WASIP1_THREADS_RUNNER`; here
the module is a `transition_dep` in wasi's configuration and the runner is an
ordinary dependency, so it is built for the host beside it. The accepted
paintings are read from `snapshots/` through `CARGO_MANIFEST_DIR`, which
`wasi_test` takes from the crate's `Cargo.toml` so that the path is the one in
the repository rather than the staged copy a rustc action compiles against.
A changed painting fails the test unless `UPDATE_SNAPSHOTS` is set, which
`./scripts/verify` does through the test executor's `--env`, and the runner
hands it to the guest.

`block-editor-plugin` has a `test` of the same shape: it is the guest half of
the plugin framework, and its tests are behind `cfg(target_arch = "wasm32")`.

Cranelift is most of what a plugin test run costs - about forty seconds a
module, and there are thirty-four of them - so compiling one is an action
rather than something each test process does again. `plugin-test-runner
--precompile-to` writes the artifact where buck2 asks for it and the test
command names it with `--precompiled`, rather than the runner looking beside
the module the way it does under cargo: what decides staleness there is an
mtime, and buck2 does not preserve those. It does not need to, because the
runner is an input to the action that wrote the artifact. Being an action is
also what makes the compile shared - once per module rather than once per test
process, and answered by the cache on a machine that has never built the
plugin. It is the difference between `./scripts/buck test //crates/...` taking
twenty-five minutes on an unchanged tree and taking twenty seconds.

The artifact is compiled for the x86_64 baseline rather than for the machine
that compiled it. Left to itself wasmtime uses every CPU feature it finds, and
the artifact then only loads on a machine with the same ones - but it is
compiled on a worker and loaded here, and the two rarely match. Naming the
target with `--target` is what makes wasmtime stop looking.

## What is still cargo's

- **Every platform but Linux on x86_64.** `reindeer.toml` configures one
  platform and a worker is one machine. macOS, Windows and Android builds, the
  release artifacts CI uploads, and `./scripts/build` and `./scripts/run` are
  cargo's. Adding a platform here is adding it to `reindeer.toml`, re-running
  `./scripts/buckify` and fixing what its build scripts need, and a worker pool
  for it where it cannot be cross-compiled.
- **The embedded browser.** `block-app`'s `web-view` feature needs WebKitGTK and
  the GTK stack under it, which the container does not have and which is far
  too large to hand a worker the way ALSA is handed. A container image of our
  own with it installed is the way to close this, and would also let beui's
  renderer tests and the plugin tests' GPU half move to a worker, with Mesa's
  software Vulkan in it.
- **The lint pass.** clippy lints the embedded browser, above, and rustfmt and
  `fix-rust-source` have no buck2 story yet.
- **No web bundle.** The third-party half of it is there: `wgpu`, `wgpu-core`,
  `wgpu-hal` and `eframe` all build for `buck/platforms:wasi`, with the app's
  feature set. What is left is `block-app` for wasm, wasm-bindgen after it, and
  the JS shims in `scripts/internal/web`.
- **Nothing stages the plugins.** `scripts/internal/common.sh` copies each
  manifest beside the modules as `<id>.plugin.json` and writes a `plugins.json`
  index for the browser and Android. There is no buck2 rule for that yet, which
  is what running `block-app` from buck2 is waiting on.
- **`BLOCK_APP_COMMIT` is `unknown`.** Under cargo, `block-app`'s build script
  asks git for the commit; a worker has no checkout to ask. Passing it in as
  config would rebuild the app on every commit, which is only worth it once
  buck2 builds something that ships.

## Where the tools come from

buck2 and starlark_fmt are prebuilt releases, checked against sha256 hashes
pinned in `scripts/internal/common.sh` the way every other tool here is. Upstream
publishes them as GitHub release assets with no hashes beside them, so the bytes
are mirrored next to this repository's other archives and the hash is what says
they are the right ones.

The pin is a dated release tag. `latest` is a tag upstream repoints on every
push to main, so it is never what to pin. A buck2 binary carries the prelude it
was built with, which is why moving that version is a change to verify with a
build rather than a number to bump.

reindeer publishes no releases at all, so it is pinned by commit in
`buck/cargo/BUCK` and built from source on a worker. Only `./scripts/buckify`
uses it; the file it writes is checked in.

## Why reindeer, and why it reads the workspace manifests

reindeer resolves the same dependency graph cargo does, from the same
`Cargo.toml` files and the same `Cargo.lock`, and writes one Buck rule per
crate. Pointing it at the workspace manifest rather than at a second manifest of
its own is what keeps cargo the single place a dependency is declared: adding
one is `cargo add`, then `./scripts/buckify`.

Where reindeer and cargo do differ is features. cargo resolves features once for
a whole build, so a feature a dev-dependency turns on is on for the library too;
buck2 builds one target per rule and unifies nothing. Where that matters the
feature is named in a fixup - `wgpu` and `wgpu-core` both carry `noop` for this
reason. A test that fails under buck2 and passes under cargo with a missing
method or a missing `cfg` is almost always this.

The other difference is what reindeer will not look at. It walks a workspace
member's ordinary dependencies and stops at `[dev-dependencies]`, so a crate
nothing but a test uses gets no target at all, and neither does an optional
dependency behind a feature only a test asks for. `crates/test-third-party`
names those - `naga`, and `wat` through `wasmi` and `wasmtime` - and has no code
and no dependents. The `[dev-dependencies]` line that wants one stays where it
belongs, on the crate whose tests use it.
