# buck2

How the workspace is built with buck2: the commands, adding crates, dependencies and other
targets, the platforms, what the bundles contain, and how the build is laid out. Read it before
changing a `BUCK` file, `Cargo.toml` or anything under `buck/`. Running tests is in
`guides/testing.md`; the build servers are in `guides/build_server.md`.

Every action runs on a remote execution server (Namespace's unless `BE3_BUILD_SERVER` picks
another). Every command is a buck2 target started through `./scripts/buck`, except
`./scripts/verify` and `./scripts/ci`, which are scripts so that each is one buck2 command.

cargo builds nothing. `Cargo.toml` is still the one place a dependency is declared, and buck2
reads it through cargo's own plans.

## Commands

| Command | What it does |
|---|---|
| `./scripts/buck run //:check` | rustc's check pass over every first-party target, host and wasm, and over what the desktop programs build for Windows and macOS |
| `./scripts/verify` | autofixes, lints, tests and plugin tests; `--check` writes nothing; `--lint`, `--tests`, `--plugin-tests` run one part |
| `./scripts/ci` | what CI runs: `./scripts/verify`; `--android DIR` also the signed APKs, `--previews BASE OUT` the paint previews |
| `./scripts/buck run //crates/block-app:native` | the app in a window, with every plugin beside it |
| `./scripts/buck build //crates/block-app:native-dist --out DIR` | a platform's release of the app |
| `./scripts/buck build //crates/be-server:dist --out DIR` | the server's release |
| `./scripts/buck build //crates/block-app:plugins --out DIR` | the plugins alone, shared by every platform |
| `./scripts/buck build //crates/block-app:web --out DIR` | the web bundle with every plugin (`:web-dist` without) |
| `./scripts/buck run //crates/block-app:web` | the web bundle and `be-server`, on http://127.0.0.1:8080 |
| `./scripts/buck run //crates/block-app:android -- --install` | the APK, signed with this machine's key, installed and started |
| `./scripts/buck build //crates/block-app:android-dist` | CI's APK, signed on a worker with CI's key |
| `./scripts/buck run //crates/beui-demo:native` | beui's demo in a window |
| `./scripts/buck run //crates/beui-web-demo:web` | beui's demo in a browser, drawn with DOM elements, on http://127.0.0.1:8070 |
| `./scripts/buck run //crates/beui:example-survey` | a crate example; every example is `:example-<name>` |
| `./scripts/buck run //:fuzz` | every fuzz target, taking turns on every core, until stopped (see Fuzz targets) |
| `./scripts/buck run //crates/sequence:fuzz-sequence -- FILE` | one fuzz target; with an input file, runs it once |
| `./scripts/buck run //:buckify` | regenerates `buck/cargo/crates.bzl` and `Cargo.lock` from the manifests |
| `./scripts/buck run //:rust-project` | writes `rust-project.json` for rust-analyzer |
| `./scripts/buck run //:lock-sysroot` | re-resolves `buck/sysroot/packages.bzl` |

Tests (`buck test`, `./scripts/test`, `:test-run`) are in `guides/testing.md`. The benchmarks
(`:example-text-bench`, `:example-open-bench`) are in `guides/performance.md`. The `:dev`
targets are in `guides/running/drive.md`.

### Options

- `--target-platforms root//buck/platforms:<p>` builds for another platform (see Platforms).
- `-m release` builds with cargo's release profile, on whichever platform. The profile is the
  `root//buck/constraints:release` constraint, added by a configuration modifier in the root
  `PACKAGE` rather than a buckconfig value, so one build can hold both profiles and every
  transition keeps it.
- Every other build is cargo's dev profile with `Cargo.toml`'s `[profile.dev.package]`
  overrides (the optimised cranelift and crypto crates), which `crates.bzl` carries as each
  crate's rustc flags.

## Target names

Target names are kebab-case, and a role has one name in every package:

| Target | Role |
|---|---|
| `:bin` | a crate's binary |
| `:test` | its tests |
| `:test-run` | the cached action that runs its tests (`guides/testing.md`) |
| `:example-<name>` | an example |
| `:fuzz-<name>` | a fuzz target |
| `:wasm` | a guest library |
| `:module` | a guest library's module |
| `:native`, `:web`, `:android` | an app's bundle per platform, which `buck2 run` runs, serves or installs here |
| `:native-dist`, `:web-dist`, `:android-dist` | the build CI publishes |
| `:dev` | the app headless, for an agent to drive |

## Adding things

### A dependency

1. `cargo add` it, or edit `Cargo.toml`.
2. `./scripts/buck run //:buckify` to regenerate the rules.

A build script runs as it does under cargo, and the libraries it links or compiles reach the
link. One that needs something from the build, such as a sysroot's pkg-config, gets it from a
fixup in `third-party/rust/fixups.bzl`.

### A crate

1. Add it to the workspace.
2. Write a `BUCK` beside its `Cargo.toml`.
3. Run `./scripts/buck run //:buckify`.

```
load("@root//buck/cargo:defs.bzl", "cargo_binary", "cargo_library", "cargo_test")

cargo_library()

cargo_test()

cargo_binary()
```

- Dependencies, features, edition and roots come from `crates.bzl`.
- The library is named after the package, its tests are `:test`, its binary `:bin`.
- An example is `cargo_example("<name>")`, which makes `:example-<name>`.
- What `Cargo.toml` cannot say is passed as arguments: `srcs` for non-Rust files the crate
  reads at compile time (default `src/**/*.rs`), `env`, `extra_deps`, or any rule attribute,
  such as a test's `remote_execution = "disabled"`.

### An editor

`crates/editors/<name>/BUCK` is one `editor(module)` call from `buck/wasm/defs.bzl`. It makes
the guest cdylib `:wasm` (linked against the plugin SDK, `guides/plugins/sdk.md`), its
`:module`, its `:manifest` and its wasm `:test`. The app picks up every editor by itself. The
whole procedure is `guides/plugins/adding_an_editor.md`.

### A fuzz target

1. In a crate whose `BUCK` calls `cargo_fuzz()` (with `extra_features` for the library features
   it needs), add `fuzz/<name>.rs`.
2. Make it a `#![no_main]` crate defining `LLVMFuzzerTestOneInput` over the library. Example:
   `crates/sequence/fuzz`.

It becomes `:fuzz-<name>`, and `//:fuzz` finds it by itself (`buck/dev/fuzz.bxl`).

- It is built with rustc's sancov instrumentation and linked with LLVM's libFuzzer, on the
  stable compiler, without a sanitizer, for Linux x86_64 only.
- `:fuzz-<name>-test` fuzzes for a moment from a fixed seed.
- Its corpus and crashes are kept in `target/fuzz/<crate>/<name>`, so it picks up where it was
  when started again.
- `//:fuzz` gives each target `BE3_FUZZ_SLICE` seconds at a time (600 by default).

### A system library

1. Add a line to `buck/sysroot/BUCK`.
2. `./scripts/buck run //:lock-sysroot`.

A `-sys` crate finds it through the fixup `_PKG_CONFIG`, which answers its build script's
pkg-config from the sysroot.

## Platforms

| Platform | Built against | Linked with |
|---|---|---|
| Linux x86_64 (default) | `buck/sysroot:amd64` | lld |
| `linux_arm64` | `buck/sysroot:arm64` | lld |
| `macos_arm64`, `macos_x86_64` | MacOSX15.5 SDK (`buck/tools:macos-sdk`) | `ld64.lld` |
| `windows_arm64`, `windows_x86_64` | MSVC runtime and Windows SDK via xwin | `lld-link`, `llvm-lib` |
| `android_arm64` | NDK r29, API 26 | lld |
| `wasi` | the WASI sysroot | rust-lld (block-app on the web) |
| `wasi_guest` | the WASI sysroot | rust-lld through `crates/wasm-link` (the plugins and their SDK) |
| `wasm32` | nothing | rust-lld (the games, the gpu shim) |

- All of them build on the same Linux workers.
- Everything is built for Linux x86_64 wherever it is asked for (`.buckconfig`'s default target
  platform), so a Mac or Windows machine shares CI's cache. `run` is the exception: on another
  machine it builds for that machine unless the command names `--target-platforms`.
- CI builds and ships only the Android app and launcher (`:android-dist`); the other platforms
  are built by hand.
- Apple's SDK licence allows it on Apple hardware only, so release builds for macOS must move
  to a Mac or Asahi worker before anything ships. xwin accepts Microsoft's Build Tools licence.
- `wasi` and `wasi_guest` are the same triple, but the app links a real wgpu backend and a
  plugin only wgpu's `custom` one, and the two cannot be unified. A `guest` constraint
  (`buck/constraints`) tells them apart, and `crates.bzl` has a plan for each.
- A native target depends on a wasm one through a transition in `buck/wasm/defs.bzl`. For
  example, a game's test says `env = {"GAME_WASM": "$(location :module)"}`.

## Bundles and shipping

- **`:native`** stages the executable as `block-app`, and every editor's manifest
  (`<id>.plugin.json`), data (`data/<id>/`), module and `.cwasm`, and the plugin SDK as
  `sdk-<SHA-256>.wasm` with its `.cwasm`.
  - Each `.cwasm` is precompiled in an action per module, for the platform the app is built
    for, by `//crates/plugin-test-runner:precompiler`, which is always the Linux x86_64 build
    the workers can run.
  - `plugins.json` lists the manifests. Every platform finds its plugins through it: beside
    the executable, in the APK's assets, or at the bundle root.
- **`:native-dist`** is a platform's release, `:plugins` the plugins once for all, and
  `//crates/be-server:dist` the server with its build id.
- **`:web`** is block-app for `wasi` and `block-gpu-shim` for `wasm32`, each run through
  `wasm-bindgen` (`buck/cargo:wasm-bindgen`, pinned to `Cargo.lock`'s version), with the page
  and shims from `crates/block-app/web`. `buck2 run` on it runs Caddy (`buck/tools:caddy`) with
  `web/Caddyfile` and be-server behind it; `-- --domain DOMAIN` serves that domain over https,
  which is how the app is deployed (`guides/hosting.md`).
- **The APK** is assembled on a worker without Gradle (`buck-tools apk`): aapt2, javac and d8,
  block-app's `[cdylib]` and `libc++_shared.so`, the plugins precompiled for arm64 (only the
  `.cwasm`s), and `zipalign -P 16`.
  - The Java is the app's own and beui's (`//crates/beui-adapter-android:android-java`),
    compiled against the platform alone: the APK carries no libraries.
  - The host's wasmtime has cranelift's arm64 backend for the arm64 precompiles (a fixup on
    `cranelift-codegen`).
  - `:android` is the APK unsigned; `buck2 run` on it signs it locally with
    `target/android-debug.keystore`, made on first use, into `target/android/block-app.apk`.
  - `:android-dist` signs on a worker with CI's keystore (`guides/build_server.md`, CI's
    Android keystore), with its own application id and label (`com.be3.block.ci`,
    `Block (CI)`) so it installs beside a local build.
- **`crates/be-launcher`** is an APK too. It downloads and installs the APKs CI uploads to
  be3-ci (`:android-dist` of block-app and of itself) and opens the app. The `publish-android`
  job in `ci.yml` says how they are stored.
- **macOS** builds are an executable and its libraries; the `.app` bundle comes with
  distribution.

## The build id

Which build a program is lives in a file beside it, never in its code: `//buck/app:build-id`
is the file `buck/app/build-id`, which git ignores.

- Every bundle carries it: `:native`, `:native-dist`, `:web`, `:web-dist`, the APK's assets,
  be-cli's `:native` and be-server's `:dist`.
- A client and a server refuse each other unless their ids match. A program that finds no id
  refuses to connect or serve. The app shows the id as its build.
- Every `./scripts/buck` command that builds or queries first writes the working tree's hash
  into the file, if it changed. The hash is taken through a throwaway index, so untracked files
  count and the same tree always gives the same id. `BE3_BUILD_ID` overrides it.
- The id is the hash of the tree, not of a commit, so two commits of the same tree (a pull
  request's head and the merge queue's commit of it) share every cached bundle, APK and smoke
  test.
- Only the bundles read the file, so a new id rebuilds nothing else.

## What `./scripts/buck` does

It runs under bash on Linux, macOS (bash 3.2) and Git Bash on Windows, and does this before
handing over to the pinned buck2:

- **Installs buck2.** The release pinned in `scripts/internal/common.sh` goes into
  `target/tools/buck2-<version>/` the first time it is missing. A `buck2` on `PATH` is never
  used, since each buck2 carries its own prelude.
- **Configures the build server.** `BE3_BUILD_SERVER` (or the prompt on the first run) picks
  it, and its hosts and key go into a generated `.buckconfig.local`, with the server's name as
  `be3.build_server`. For Namespace it also installs and runs `nsc`. Details:
  `guides/build_server.md`.
- **Relays through an HTTPS proxy.** buck2's remote execution client dials the server's hosts
  directly and never reads `HTTPS_PROXY`. When it is set, `./scripts/buck` builds
  `scripts/internal/re-relay` with Go (1.24 or newer) and keeps two running, one for the
  executor and one for the storage, on two ports of `127.0.0.1` picked from the checkout's path
  (so two checkouts on one machine almost never share one), and points buck2 at them in
  `.buckconfig.local`. Each relay sends its calls on through the proxy, over HTTP/1.1 if that
  is all the proxy speaks. Their errors go to `target/re-relay.log`. A `.buckconfig.local` a
  person wrote is left alone, and the relays are not used then.
- **Writes the build id** (see The build id).
- **Retries infrastructure errors.** buck2 exits with 2 for one, such as a connection to the
  build server resetting partway through a download, and does not retry it itself. The wrapper
  runs such a command again, three times in all (`BE3_BUCK_ATTEMPTS`); finished actions are
  cached by then. For `run` it builds first (`run --command-args-file`; Windows refuses
  `--emit-shell`) and retries that, since the program's own exit status is `run`'s.
- **Lets `test` put tests on the workers** (`guides/testing.md`).

## Layout

| Path | What it holds |
|---|---|
| `.buckconfig` | cells and the execution platform; the prelude is the one bundled in the buck2 binary |
| `BUCK.v2` | the `//:` commands, with their scripts in `buck/dev` (`BUCK.v2` because a case-insensitive filesystem cannot hold `BUCK` beside `buck/`) |
| `buck/tools` | every compiler and tool, downloaded and pinned by hash and size |
| `buck/toolchains` | the toolchains built from them, per target platform |
| `buck/platforms` | the execution platform (a build server's worker) and every target platform; `cross.bzl` lists the cross-compiled ones and the plans `crates.bzl` keys them by, and everything else that lists platforms reads it |
| `buck/constraints` | the `release` and `guest` constraints |
| `buck/sysroot` | the Ubuntu 24.04 packages everything is compiled against |
| `buck/cargo` | the BXL that writes `crates.bzl`, and the macros that read it: `defs.bzl` for workspace crates, `third_party.bzl` for the rest |
| `buck/wasm` | editors, plugin tests and the rules that build wasm modules |
| `buck/app`, `buck/android` | how the app, the web bundle and the APK are laid out |
| `buck/dev` | the `//:` commands' scripts, `verify.bxl`, `fix.sh`, `workspace.bxl`, `fuzz.bxl` |
| `third-party/rust` | every third-party crate as `<name>-<version>`, and `fixups.bzl`: what cargo's plans cannot say about a crate (environment for its build script, extra rustc flags, a file to overlay) |
| `crates/<crate>/BUCK` | one per crate, written by hand |
| `crates/buck-tools` | the build's own helpers: the `crates.bzl` generator, the sysroot resolver, the APK packer, clippy's fixer and `//:rust-project` |
| `crates/local-tools` | what runs on this machine after a buck2 command, built for it in that command: `verify`, `deploy` (`./scripts/deploy`) and `fuzz` (a fuzz target's runner) |

### `crates.bzl`

`buck/cargo/crates.bzl` is generated and checked in, like a lockfile: every crate's
dependencies, features and targets, first- and third-party, from cargo's plans, with each
third-party crate's checksum and size.

- `./scripts/buck run //:buckify` runs `buckify.bxl` on a worker and copies the result into
  place, with `Cargo.lock` brought up to date with the manifests.
- `./scripts/verify` builds the same action and copies it too; under `--check` it fails if
  either file is out of date. Its build uses the checked-in copies, so a stale one is fixed by
  the run after.
- The action is keyed on the manifests, `Cargo.lock`, the paths cargo discovers targets at and
  `crates/buck-tools`, so it is shared through the cache.

## How `./scripts/verify` works

It is one `buck2 bxl` of `buck/dev/verify.bxl`, which builds everything with `--keep-going`
and prints what is read afterwards: the generated files, the paintings, each crate's fixes,
and `crates/local-tools` built for this machine. The script then runs that `verify`, which
reads the rest and writes it into the checkout.

Anything that runs on this machine after a buck2 command belongs in `crates/local-tools` as a
subcommand, not in bash or a crate of its own, so it works the same on Linux, macOS and Git
Bash on Windows and builds once.

- **Autofixes** are actions (`buck/dev/fix.sh`), one per crate, on a copy of the crate's
  sources, output as the files that changed:
  1. clippy's machine-applicable suggestions from every target's `[clippy.json]` (none if the
     crate failed to compile, since a `view!` that does not parse leaves its imports looking
     unused);
  2. fix-rust-source;
  3. rustfmt.

  A fix that makes another possible shows up on the next run.
- **Clippy** reads every target's `[clippy.json]` in the configurations `//crates/...`
  resolves to and, like `//:check`, in `windows_x86_64` and `macos_arm64` for the desktop
  programs' first-party dependencies (`rust_targets` in `buck/dev/workspace.bxl`). So code
  behind `cfg(windows)` or `cfg(target_os = "macos")` is checked without a link or a test for
  either. `clippy.toml` is empty but must exist.
- **File modes.** A file's executable bit is part of every action that reads it, and a Windows
  checkout has none, so no file a build reads may have one, or Windows would miss every cache
  entry Linux wrote. Scripts are run with `sh`, and verify's lint clears the bit from anything
  outside `scripts/`.

## rust-analyzer

`./scripts/buck run //:rust-project` writes `rust-project.json` (git ignores it), which
rust-analyzer prefers over `Cargo.toml`.

- It covers the host and, as `wasm32-wasip1-threads`, the plugins; takes the standard library
  and proc-macro server from the pinned toolchain; and checks on save through buck2.
- Run it again after adding a crate or dependency.
- Use the rust-analyzer an editor extension ships: the toolchain's own panics on this
  workspace.

## Things that are not obvious

- **Features.** Every crate has the features cargo's plan for the platform gives it, and the
  plans are `cargo test` for the host and each cross platform, so what a dev-dependency turns
  on is on for the library too, as under cargo. What cargo builds for the machine running the
  build (proc macros, build scripts and their dependencies) is one build here, the
  `linux-x86_64` one, with the union of what every plan asks of it.
- **The compiler stays stable.** `nightly_features = False` in `buck/toolchains/BUCK`.
  Otherwise the prelude sets `RUSTC_BOOTSTRAP=1`, and `cfg(target_feature = "atomics")` on
  `wasm32-wasip1-threads` then answers differently from cargo, which breaks wgpu's
  `Send`/`Sync` checks.
- **Downloads.** Every `http_archive` has `size_bytes` as well as `sha256`; without it buck2
  sends a HEAD request per download on every new daemon.
