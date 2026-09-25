# Bazel

Bazel builds, lints and tests the workspace, and every action runs on
BuildBuddy's remote workers. Every command is a Bazel target started through
`./scripts/bazel`. cargo builds nothing; `Cargo.toml` is still the one place a
dependency is declared, and Bazel reads it through cargo's own plans.

## Commands

| Command | What it does |
|---|---|
| `./scripts/bazel run //:check` | rustc's check pass, through clippy, over every first-party target, host and wasm |
| `./scripts/bazel run //:verify` | autofixes, lints, tests and plugin tests; `-- --check` writes nothing, `-- --lint`, `--tests`, `--plugin-tests` run one part |
| `./scripts/bazel test //crates/...` | the tests alone |
| `./scripts/bazel test //crates/editors/checklist:test` | one editor's tests; add `--test_env=UPDATE_SNAPSHOTS=1` to accept its paintings |
| `./scripts/bazel run //crates/block-app:app` | the app, with every plugin beside it |
| `./scripts/bazel run //crates/block-app:smoke` | the app for ten seconds in a virtual display |
| `./scripts/bazel build //crates/block-app:dist` | a platform's release (with `--config=release`): app, `be-server`, PDFium |
| `./scripts/bazel build //crates/block-app:plugins` | the plugins alone, shared by every platform |
| `./scripts/bazel build //crates/block-app:web` | the web bundle with every plugin (`:web-dist` without) |
| `./scripts/bazel run //crates/block-app:web-serve` | the web bundle and `be-server`, on http://127.0.0.1:8080 |
| `./scripts/bazel run //crates/block-app:android -- --install` | the APK, signed with this machine's key, installed and started (`build :android-dist` is CI's: no plugins, signed on a worker with CI's key) |
| `./scripts/bazel run //crates/beui:demo-example` | a crate example; every example is `<name>-example` |
| `./scripts/bazel run //:rust-project` | writes `rust-project.json` for rust-analyzer |
| `./scripts/bazel run //:lock-sysroot` | re-resolves `bazel/sysroot/packages.bzl` |

What a build writes is under `bazel-bin/`, at the target's package: `:web` is
`bazel-bin/crates/block-app/web-bundle`. Only what the command names is
downloaded (`--remote_download_toplevel`).

`--platforms=//bazel/platforms:<p>` builds for another platform, and
`--config=release` is cargo's release profile (Bazel's `opt` compilation
mode), which every transition keeps. A release build takes
`--//bazel/config:commit=SHA`, the commit the app reports; a dev build says
`unknown` whatever it is passed, so a new commit does not rebuild it.

## What `./scripts/bazel` does

It runs under bash on Linux, macOS (bash 3.2) and Git Bash on Windows, and puts
these in front of the pinned Bazel:

- **Bazel itself.** The release pinned in `scripts/internal/common.sh` (and in
  `.bazelversion`) is installed into `target/tools/bazel-<version>/` the first
  time it is missing, checked against its hash. A `bazel` or bazelisk on
  `PATH` is not used.
- **The BuildBuddy key**: `BUILDBUDDY_API_KEY`, else `.buildbuddy-api-key` at
  the root (git ignores it), else `~/.config/be3/buildbuddy-api-key`. With
  none, a person at a terminal is asked for it and it is saved to
  `.buildbuddy-api-key`; without a terminal it fails and says what to set.
  Bazel reads the key through `scripts/internal/buildbuddy-credentials`, a
  credential helper `.bazelrc` names, so it is never on a command line.
- **An HTTPS proxy.** Bazel's remote execution and build event clients dial
  BuildBuddy directly and never read `HTTPS_PROXY`. When it is set,
  `./scripts/bazel` builds `scripts/internal/re-relay` with Go (1.24 or
  newer), leaves it running on `127.0.0.1:18980`, and writes
  `target/re-relay.bazelrc` (which `.bazelrc` imports) pointing Bazel at it;
  the relay sends each call on through the proxy, over HTTP/1.1 if that is all
  the proxy speaks. It still needs a key, though a proxy that adds
  BuildBuddy's header itself accepts any value, such as
  `BUILDBUDDY_API_KEY=proxy-injected`. Its errors go to `target/re-relay.log`.
- **Platforms.** Everything is built for Linux x86_64 wherever it is asked for
  (`.bazelrc`'s `--platforms`), so a Mac or Windows machine shares CI's cache.
  `run` is the exception: on another machine it builds for that machine unless
  the command names `--platforms`.
- **File modes.** A file's executable bit is part of every action that reads
  it, and a Windows checkout has none, so no file a build reads may have one:
  Windows would miss every cache entry Linux wrote. Scripts are run with `sh`
  (`sh_command` in `bazel/app/defs.bzl`), and `//:verify`'s lint clears the bit
  from anything outside `scripts/`.
- **One retry.** Bazel exits with 34, 36, 38 or 39 for an infrastructure error,
  such as BuildBuddy resetting a download partway. The wrapper runs such a
  command once more; the actions are cached by then. For `run` it builds first
  (`run --script_path`) and retries that, since the program's own exit status
  is run's, and then runs the program without Bazel holding its lock, which is
  what lets `//:verify` call Bazel itself.

BuildBuddy's invocation page for every command is printed at its end, with the
build's metadata, since Bazel streams its build events there. The last
command's remote calls are in `target/bazel-remote-grpc.log` and its actions in
`target/execution_log.binpb.zst`, which is what BuildBuddy asks for when a
build goes wrong.

## Layout

- `MODULE.bazel`: the modules the build uses, the Rust toolchain, and the
  cargo extension. `MODULE.bazel.lock` records what the extensions made and is
  checked in.
- `.bazelrc`: BuildBuddy, the platforms, and the flags every command takes.
- `BUILD.bazel`: the `//:` commands, scripts in `bazel/dev`.
- `bazel/tools`: every compiler and SDK, fetched and pinned by hash on a
  worker (`fetch` in `defs.bzl`).
- `bazel/toolchains`: the C toolchains built from them, per target platform.
- `bazel/platforms`: the execution platform (a BuildBuddy worker) and every
  target platform, each with the config_setting a select() names it by.
- `bazel/sysroot`: the Ubuntu 24.04 packages everything is compiled against.
- `bazel/cargo`: the module extension that reads cargo's plans and the macros
  that write a crate's rules from them.
- `crates/bazel-tools`: the build's own helpers (the sysroot resolver, the APK
  packer, clippy's runner and fixer, and `//:rust-project`).
- `bazel/wasm`: editors, plugin tests and the rules that build wasm modules.
- `bazel/app`, `bazel/android`: how the app, the web bundle and the APK are
  laid out.
- `bazel/ci`: what CI builds and uploads.
- `third-party/rust/fixups.bzl`: what the cargo extension is told about a crate
  it cannot work out from cargo's plans alone.
- `crates/<crate>/BUILD.bazel`: one per crate, written by hand.

## Where the crates come from

`bazel/cargo/extension.bzl` asks cargo, for every platform, what it would
build: `cargo test --unit-graph` for the host and each native platform, and
the plans of the plugins, the games and the app on the web, each on its own.
From those it makes a repository per third-party crate, downloaded from
crates.io and checked against Cargo.lock's checksum, with a BUILD file that
selects each platform's features and dependencies, and `@crates//:crates.bzl`,
the same facts about the workspace's own crates, which `bazel/cargo/defs.bzl`
reads. A unit cargo builds for the build machine - a proc macro, a build
script and their dependencies - is recorded as Linux x86_64's, since that is
what the workers are.

The extension downloads the pinned cargo and rustc for this machine and runs
them here, which takes about half a minute; nothing is compiled. It runs again
only when one of its inputs changes - a `Cargo.toml`, `Cargo.lock`,
`fixups.bzl`, or the files cargo discovers targets at - and
`MODULE.bazel.lock` records the result, so a checkout whose lockfile is
current never runs it. `//:verify` brings the lockfile up to date, and fails on
a stale one under `--check`.

## Adding things

**A dependency**: `cargo add` (or edit `Cargo.toml`). The next command
regenerates the crates and updates `MODULE.bazel.lock`; commit both. A crate
whose build script cannot run on a worker as it is gets an entry in
`third-party/rust/fixups.bzl`.

**A crate**: add it to the workspace, and write a `BUILD.bazel` beside its
`Cargo.toml`:

```
load("//bazel/cargo:defs.bzl", "cargo_binary", "cargo_library", "cargo_test")

cargo_library()

cargo_test()

cargo_binary()
```

Dependencies, features, edition and roots come from `crates.bzl`. The library
is named after the package, its tests are `:test`, a binary named like the
package is `<name>-bin`, and an example is `cargo_example("<name>")`. What
`Cargo.toml` cannot say is passed as arguments: `compile_data` for non-Rust
files the crate reads at compile time, `data` and `runtime_env` for what its
tests read when they run, `env`, `extra_deps`, or any rule attribute.

**An editor**: `crates/editors/<name>/BUILD.bazel` is one `editor(name,
module)` call from `bazel/wasm/defs.bzl`, which makes the guest cdylib, its
`:module`, its `:manifest` and its wasm `:test`. The app picks up every editor
by itself.

**A system library**: a line in `bazel/sysroot/wanted.bzl`, then
`./scripts/bazel run //:lock-sysroot`. A `-sys` crate finds it through the
fixup `_PKG_CONFIG`, a pkg-config that answers from the sysroot.

## Tests

Most tests run on the workers, including beui's and be-compositor's, which draw
through lavapipe from `bazel/sysroot:amd64-test`. A test reads files at run
time from its runfiles, relative to the workspace (`CARGO_MANIFEST_DIR` is the
crate's directory, relative), so what it reads is its `data`.

**Plugin tests** run here instead, because they read and write the accepted
paintings in `snapshots/`. They are tagged `plugin` (and `local`), which is how
`//:verify`'s `--tests` and `--plugin-tests` split. Each is the crate's tests
compiled to wasm and run by `plugin-test-runner`, the host `block-app` runs a
plugin in, so they paint with the FreeType and HarfBuzz the plugin ships.

`--test_env=NAME=VALUE` sets a variable and `--test_arg=NAME` runs one test.
Rust runs a binary's tests on threads, like `cargo test`, so tests that touch
process-wide state must take turns.

## Platforms

| Platform | Built against | Linked with |
|---|---|---|
| `linux_x86_64` (default) | `bazel/sysroot:amd64` | lld |
| `linux_arm64` | `bazel/sysroot:arm64` | lld |
| `macos_arm64`, `macos_x86_64` | MacOSX15.5 SDK (`bazel/tools:macos-sdk`) | `ld64.lld` |
| `windows_arm64`, `windows_x86_64` | MSVC runtime and Windows SDK via xwin | `lld-link` |
| `android_arm64` | NDK r29, API 26 | lld |
| `wasi` | the WASI sysroot | rust-lld (block-app on the web) |
| `wasi_guest` | the WASI sysroot | rust-lld (the plugins) |
| `wasm32` | nothing | rust-lld (the games, the gpu shim) |

All of them build on the same Linux workers. CI builds `//bazel/ci:everything`
and `//crates/...` in one command: every platform's release app and dev app,
the plugins, the web bundle and the APK (`bazel/ci/BUILD.bazel`). Apple's SDK
licence allows it on Apple hardware only, so release builds for macOS move to
a Mac or Asahi worker before anything ships; xwin accepts Microsoft's Build
Tools licence.

`wasi` and `wasi_guest` are the same triple, but the app links a real wgpu
backend and a plugin only wgpu's `custom` one, and cargo plans the two as
different builds. A `wasm_role` constraint (`bazel/constraints`) tells them
apart, and each has a plan of its own.

A native target depends on a wasm one through a transition in
`bazel/wasm/defs.bzl`: a game's test says
`rustc_env = {"GAME_WASM": "$(execpath :module)"}`.

## Shipping

- `:app` stages the executable as `block-app`, PDFium, and every editor's
  manifest (`<id>.plugin.json`), module and `.cwasm`, precompiled in an action
  per module for the platform the app is built for, by
  `//crates/plugin-test-runner`, built for the workers.
- `:dist` is what CI ships per platform, and `:plugins` the plugins once for all.
  `//bazel/ci:artifacts` is all of what CI uploads, in one directory.
- `:web` is block-app for `wasi` and `block-gpu-shim` for `wasm32`, each run
  through `wasm-bindgen` (`bazel/cargo:wasm-bindgen`, pinned to `Cargo.lock`'s
  version), with the page and shims from `crates/block-app/web` and a
  `plugins.json` the browser finds the plugins through. `:web-serve` runs Caddy
  (`bazel/tools:caddy`) with `web/Caddyfile`; a deployment uses the same
  Caddyfile with `BE3_DOMAIN_NAME` and `BE3_WEB_ROOT`.
- The APK is assembled on a worker without Gradle (`bazel-tools apk`):
  aapt2, javac and d8, block-app's cdylib and `libc++_shared.so`, the
  plugins precompiled for arm64, and `zipalign -P 16`. `:android` signs it
  locally with `target/android-debug.keystore`, made on first use.
  `:android-dist` signs on a worker with CI's keystore, which BuildBuddy keeps
  as the secret `ANDROID_DEBUG_KEYSTORE_BASE64` and passes only to actions on
  the `android_signing` execution platform, with its own application id and
  label (`com.be3.block.ci`, `Block (CI)`) so it installs beside a local build.
  After changing the secret, bump `key_version` in `crates/block-app/BUILD.bazel`.
  The host's wasmtime has cranelift's arm64 backend for the arm64 precompiles
  (a fixup on `cranelift-codegen`).
- The macOS builds are an executable and its libraries; the `.app` bundle
  comes with distribution.

## rust-analyzer

`./scripts/bazel run //:rust-project` writes `rust-project.json` (git ignores
it), which rust-analyzer prefers over `Cargo.toml`: rules_rust's generator,
run for the host's crates and again for the plugins as
`wasm32-wasip1-threads`. It downloads everything it describes, which is
every crate's build output: several gigabytes. Run it again after adding a
crate or dependency.

## Things that are not obvious

- **Features.** Each platform's features are cargo's for that platform's plan,
  so a feature a dev-dependency turns on is on for the library too, as it is
  under `cargo test`. A crate cargo builds for the build machine takes the
  union of what every plan asks of it there.
- **The compiler stays stable.** Nothing sets `RUSTC_BOOTSTRAP`; the cargo
  extension uses it only for `--unit-graph`, and compiles nothing.
- **Output paths.** `--experimental_platform_in_output_dir` names each output
  directory after its platform, so the same target built for two platforms in
  one command has two paths.
- **Downloads.** The compilers, SDKs and sysroots are fetched in actions on a
  worker and cached there (`bazel/tools/defs.bzl`), so a machine that asks for
  a build downloads none of them. Rust and the crates' sources are Bazel
  repositories, fetched here.
- **The key.** A wrong or missing key looks like a build with no cache hits,
  not an error; the invocation page says which.
- **Clippy** runs through rules_rust's clippy aspect, driven by
  `crates/bazel-tools`' clippy over every configuration a first-party crate is
  built in, including the wasm ones, which is why wasm-only code is linted.
  `clippy.toml` is empty but must exist.
