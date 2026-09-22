# buck2

The repository builds with cargo. buck2 builds part of it as well, and is being
grown towards the rest. This guide says what it covers today, how to run it, and
what has to happen for it to cover the rest.

Nothing here replaces cargo. `./scripts/check`, `./scripts/verify`,
`./scripts/build` and `./scripts/run` are unchanged, and a change that does not
touch a `BUCK` file needs nothing from this guide.

## What buck2 builds today

- Every third-party crate the workspace depends on, on `x86_64-unknown-linux-gnu`:
  around nine hundred of them, including the ones that compile C or assembly -
  sqlite, zstd, freetype, harfbuzz, tree-sitter, ring, wasmtime.
- Thirty-three first-party crates and their unit tests, which is the native half
  of the workspace: the `be-*` stack, `block*`, `beui`, `reactive`,
  `text-editor-core`, the games' api and host, and the tools.
- The three game modules, as WebAssembly, and the native tests that drive them
  through the host.
- All thirty-three editors, as the wasm plugins the app loads, and their tests,
  compiled to wasm and run through the same host `block-app` runs a plugin in.

`./scripts/buck test //crates/... --exclude cargo-only` is 68 test targets.

## Running it

```
./scripts/buck build //crates/...
./scripts/buck test //crates/... --exclude cargo-only
./scripts/buckify
```

`./scripts/buck` is buck2 with this machine written down first: it resolves the
host tools into `buck/tools` and, if this machine has the credential for the
shared cache, writes it to `.buckconfig.local`. Both are ignored by git. Always
go through it rather than calling `buck2` directly; a checkout that has never
run it has no `buck/tools` and will not load.

`./scripts/buckify` regenerates `third-party/rust/BUCK`. Run it after changing
any `Cargo.toml` or `Cargo.lock`; CI fails if the file and the manifests
disagree.

The tools are installed by `./scripts/internal/install-buck2.sh`,
`install-starlark-fmt.sh` and `install-reindeer.sh`. The first two are
downloads; reindeer is built from source, which takes a few minutes.

## The shared cache

CI builds every action and writes the result to a cache, so a checkout that has
never built anything gets the answers rather than the work:

```
$ ./scripts/buck build //crates/...
Cache hits: 96%
Commands: 4713 (cached: 4544, remote: 0, local: 169)
Network: up 2.6MiB  down 369MiB

real    0m25.283s
```

Twenty-five seconds against about ten minutes of compiling. Nothing runs on a
remote machine; buck2 only asks whether an action's result is already known, and
what comes back is a digest rather than a file. Buck2 downloads an artifact only
when a local action needs it or when it is something you asked to build, so a
build that hits everywhere never sees the intermediate rlibs at all - which is
where most of the saving is. Asking for one binary rather than `//crates/...`
downloads far less than the 369MiB above.

To use it, put the credential in the environment:

```
export BLOCKS_CACHE_AUTH=$(printf 'bazel:<password>' | base64 -w0)
```

Without it nothing is configured and the build is local-only, which is what a
fresh checkout does rather than failing against a server it cannot talk to.
Reads are all a developer needs; `BLOCKS_CACHE_UPLOAD=1` turns on writes and is
CI's, because a cache anyone can write to is a cache anyone can use to hand
every machine that reads it a compiler output of their choosing.

The server is a [bazel-remote](https://github.com/buchgr/bazel-remote) speaking
the remote execution API, with no scheduler and no workers. `buck/platforms/BUCK`
is the execution platform that turns the cache on; `[buck2] digest_algorithms`,
`sqlite_materializer_state` and `default_allow_cache_upload` in `.buckconfig`
are the three settings without which it silently does nothing.

A clean local build fills about twelve gigabytes of `buck-out`, which is roughly
what cargo's `target/` costs for the same workspace. `buck2 clean` empties it.

## Why buck/tools exists

Two machines share a cache entry when they agree on an action, and an action is
its command line plus the files it reads. Everything a toolchain gives buck2 as
a plain string ends up in that command line, so a toolchain that names
`/usr/bin/clang` makes every action using it machine-specific, and the cache
useless across machines. The prelude's Python is worse: it is in the command
line of every action there is.

So the host tools reach buck2 as files instead. `./scripts/buck` writes a
one-line wrapper per tool into `buck/tools`, and `buck/toolchains/BUCK` hands
those to the toolchains as artifacts, which buck2 writes into a command line as
a path relative to the repository root - the same string everywhere.

Each wrapper names the version of the tool it runs:

```sh
#!/bin/sh
# Ubuntu clang version 18.1.3 (1ubuntu1)
exec clang-18 "$@"
```

That line is the other half. buck2 keys an action on the bytes of every file it
reads, so the version makes a cache entry belong to a compiler rather than to a
machine: two machines on the same compiler share it, and two on different
compilers do not. Without it a build would take an artifact compiled by
something else and never say so. What it costs when they differ, measured by
editing a version line and rebuilding:

| | cache hits |
|---|---|
| same tools | 96% |
| a different clang | 89% |
| a different rustc | 54% |

`buck/tools/python3` is the exception, and the only wrapper checked in. It has
no version and resolves the interpreter itself at run time, so its bytes are
the same on every machine. That is deliberate: this Python decides nothing
about what an artifact contains. It runs the prelude's own helpers -
`rustc_action`, `buildscript_run`, `from_any_dir`, `dep_file_processor` - which
start the real compiler and shuffle paths and diagnostics around it. Keying on
it would only mean Ubuntu 24.04, which has 3.12, sharing nothing with a machine
that has 3.13. A checkout running 3.12 against a cache filled by one running
3.13 gets the same 94% it would have got from an identical machine.

What the Python version does decide is whether the helpers run at all: one of
them needs `Path.relative_to(walk_up=True)`, which arrived in 3.12.
`./scripts/buck` refuses to run without one, because the alternative is a
`TypeError` from inside an unrelated C compile.

## Is this hermetic, and does it need a build server?

No, and no.

A hermetic toolchain means the compilers are downloaded and content-addressed,
so every machine has the same ones. That needs downloads, not a server, and for
rustc it is already effectively true: `rust-toolchain.toml` pins the version and
rustup gives every checkout the same one, which is why the rustc wrapper says
the same thing everywhere without anything being downloaded.

A hermetic *build* is a bigger claim: that the whole input root is declared. It
is not, and downloading clang would not make it so. The C compiles read
`/usr/include`, and the links resolve `-lasound` and the system libc, none of
which buck2 sees. Two machines would compute the same action digest from
different inputs - which is worse than not sharing, because it is sharing that
is wrong. Completing the input root is what remote execution buys, by running
each action in a declared container image.

That is not worth a build server here. The C toolchain touches about a tenth of
the graph, the C compiles inside it never hit the cache anyway, and uploads are
CI's alone, so a machine whose C output would differ never poisons anything -
it just compiles those actions itself, which is under a minute. If heterogeneous
machines ever do become the problem, developing in the image CI uses is the
cheap answer and gets the same soundness for none of the operational surface.
Remote execution is the answer to a different question: making *misses* fast by
fanning them out.

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
instead. The wrapper for it drops `-fuse-ld=lld` on the way through, because
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

The wasm C compiler is not the host one. FreeType's setjmp lowering needs clang
19 or newer and Ubuntu 24.04 ships 18 as plain `clang`, so `./scripts/buck`
writes a second pair of wrappers for it, choosing the same compiler the cargo
build does.

**reindeer resolves the third-party crates once per platform**, and
`reindeer.toml` names all three. The platform names are not free: the prelude
maps a buck2 configuration to one of them, so `wasi` is `os:wasi` + `cpu:wasm32`
and `wasm32` is `os:none` with the same cpu.

What that costs is feature fixups. reindeer resolves one feature set per
platform across the whole workspace, so a feature one crate wants natively
arrives everywhere: `uuid`'s `v4` and `rand`'s `os_rng` both reached
`wasm32-unknown-unknown`, where there is no operating system to ask for entropy
and the crates say so with a `compile_error`. The fixups under
`third-party/rust/fixups` take them back off for that platform.

One of those fixups is a placeholder for a decision nobody has made yet.
`wgpu` on wasm is pinned to the guest's features - the plugin reaches the host's
device through the gpu abi and carries no backend of its own - and that is only
right while the one thing buck2 builds for wasm is a guest. The browser build of
the app wants the other feature set on the same platform, and no fixup can give
one crate two answers. Under cargo this is why the plugins get a cargo call of
their own. Under buck2 it wants a second third-party tree, generated by a second
reindeer config, which is the thing to design before the web bundle arrives.

**A native target can depend on a wasm one.** `buck/wasm/defs.bzl` has a rule
that transitions its dependency to a wasm platform, so a test that is built for
the host can name a module that is not:

```
env = {"GAME_WASM": "$(location :module)"}
```

That replaces the build script under cargo, which shells out to a second cargo
build for wasm32 and prints the path it wrote to. The plugin tests will want the
same rule.

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

`./scripts/verify` still lints with cargo, and should keep doing so until buck2
covers the whole workspace. Switching now would quietly stop linting the 33
editors and `block-app`, which have no BUCK files yet.

## How the build is laid out

- `.buckconfig` names the cells, the execution platform and the three cache
  settings. The prelude is the one bundled in the buck2 binary, so there is no
  submodule to check out and no prelude version to keep in step with the binary:
  the pinned buck2 release pins both.
- `buck/toolchains/BUCK` is the toolchain, built on the wrappers in
  `buck/tools`; `defs.bzl` beside it holds the three small rules that take a
  file where the prelude's own toolchains take a name. rustc gets
  `-Copt-level=0`, which is cargo's dev profile and also what makes the prelude
  tell a build script what `OPT_LEVEL` it is building for.
- `buck/platforms/BUCK` is the execution platform: the prelude's, with the
  shared cache wired in.
- `third-party/rust/BUCK` is generated by reindeer from the workspace manifests.
  It is checked in and nothing but `./scripts/buckify` should edit it. Crates
  are downloaded from crates.io at build time rather than vendored, so the
  repository carries the rules and not nine hundred crates of source.
- `third-party/rust/fixups/<crate>/fixups.toml` is what reindeer needs told
  about a crate it cannot work out on its own. Every crate with a build script
  needs one, if only to say `buildscript.run = true`.
- `third-party/system/BUCK` is for libraries this machine provides rather than
  builds. Today that is the C++ runtime a Rust binary linking harfbuzz needs.
- `crates/<crate>/BUCK` is written by hand, one per crate.

## Adding a crate

Write a `BUCK` file beside its `Cargo.toml`:

```
rust_library(
    name = "be-store",
    srcs = glob(["src/**/*.rs"]),
    crate = "be_store",
    crate_root = "src/lib.rs",
    edition = "2024",
    env = {
        "CARGO_CRATE_NAME": "be_store",
        "CARGO_MANIFEST_DIR": "crates/be-store",
        "CARGO_PKG_NAME": "be-store",
        "CARGO_PKG_VERSION": "0.1.0",
    },
    visibility = ["PUBLIC"],
    deps = ["//third-party/rust:serde"],
)

rust_test(
    name = "test",
    srcs = glob(["src/**/*.rs"]),
    crate = "be_store",
    crate_root = "src/lib.rs",
    edition = "2024",
    env = { ... },
    deps = ["//third-party/rust:serde"],
)
```

Things worth knowing when you do:

- The `env` block is cargo's package environment. First-party code reads
  `CARGO_PKG_VERSION` through `env!()` and will not compile without it.
- A dependency on another crate here is `//crates/<dir>:<package name>`; a
  third-party one is `//third-party/rust/<package name>`, spelled exactly as
  cargo spells the package.
- `rust_test` takes the library's own sources, because the tests are
  `#[cfg(test)]` modules inside them, and its `deps` are the library's plus the
  dev-dependencies.
- A proc-macro library takes `proc_macro = True`; its test target takes
  `rustc_flags = ["--extern", "proc_macro"]` instead, because `rust_test` has no
  such attribute.
- A binary target whose name matches the library's needs a different one, since
  both are targets in the same package. `block-server` is `:block-server-bin`.
- Anything the crate reads at compile time that is not Rust - a `.wgsl` shader,
  a font, an `examples/` file a test includes - has to be in `srcs`.

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
buck2 never sets `UPDATE_SNAPSHOTS`, so a changed painting fails here and is
accepted with `./scripts/verify` as before.

`block-editor-plugin` has a `test` of the same shape: it is the guest half of
the plugin framework, and its tests are behind `cfg(target_arch = "wasm32")`.

## What is not covered yet

The gaps, roughly in the order they are worth closing:

- **Only `x86_64-unknown-linux-gnu`.** `reindeer.toml` configures one platform.
  Adding macOS and Windows is a matter of adding them there, re-running
  `./scripts/buckify` and fixing what the new platforms' build scripts need; the
  first-party `BUCK` files would then need `select()` where the dependency sets
  differ.
- **Nothing stages the plugins.** `scripts/internal/common.sh` copies each
  manifest beside the modules as `<id>.plugin.json` and writes a `plugins.json`
  index for the browser and Android. There is no buck2 rule for that yet, and
  nothing to consume one until `block-app` is built.
- **No web bundle.** It is the same wasm build with wasm-bindgen after it, and
  the JS shims in `scripts/internal/web` beside it.
- **`block-app` is not built.** It has a build script that shells out to git,
  it links libghostty-vt, which Zig builds, and it is the one crate that pulls
  in the whole GTK and WebKitGTK tree.
- **One test target is labelled `cargo-only`** and skipped by the buck2 run.
  `block-plugin-api` has one test that walks `crates/editors` in the source
  tree; buck2 compiles a crate against a staged copy of its own sources, so
  `CARGO_MANIFEST_DIR/../editors` is not there. Giving it the manifests means
  naming all 33 editor packages somewhere, because they are packages of their
  own and a `glob` cannot reach into them. cargo runs it.
- **The lint pass is still cargo's.** Clippy works through buck2 (above) but
  cannot replace `cargo clippy` until every crate has a BUCK file. rustfmt and
  `fix-rust-source` have no buck2 story at all yet.
- **C compiles are never served from the cache.** They are the 4% the numbers
  above leave on the table - about twenty seconds of zstd, sqlite and freetype
  on a fresh checkout. Everything else hits. The cause is somewhere in how buck2
  keys dep-file-tracked compiles; turning on the remote dep file cache does not
  change it.
- **No remote execution.** The cache answers with results; nothing runs on
  another machine, so a miss is as slow as it ever was. That is a separate
  switch (`remote_enabled` in `buck/platforms/BUCK`) and needs workers rather
  than storage; the section above says when it would be worth it.

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

reindeer publishes no releases at all, so it is pinned by commit and built from
source. Only `./scripts/buckify` needs it; the file it writes is checked in.

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
