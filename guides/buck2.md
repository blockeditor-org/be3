# buck2

buck2 builds, lints and tests the workspace, and every action runs on
BuildBuddy's remote workers. Every command is a buck2 target started through
`./scripts/buck`. cargo builds nothing; `Cargo.toml` is still the one place a
dependency is declared, and buck2 reads it through cargo's own plans.

## Commands

| Command | What it does |
|---|---|
| `./scripts/buck run //:check` | rustc's check pass over every first-party target, host and wasm |
| `./scripts/buck run //:verify` | autofixes, lints, tests and plugin tests; `-- --check` writes nothing, `-- --lint`, `--tests`, `--plugin-tests` run one part |
| `./scripts/buck test //crates/...` | the tests alone |
| `./scripts/buck test //crates/editors/checklist:test` | one editor's tests; add `-- --env UPDATE_SNAPSHOTS=1` to accept its paintings |
| `./scripts/buck run //crates/block-app:app` | the app, with every plugin beside it |
| `./scripts/buck run //crates/block-app:smoke` | the app for ten seconds in a virtual display |
| `./scripts/buck build //crates/block-app:dist --out DIR` | a platform's release: app, `be-server`, PDFium |
| `./scripts/buck build //crates/block-app:plugins --out DIR` | the plugins alone, shared by every platform |
| `./scripts/buck build //crates/block-app:web --out DIR` | the web bundle with every plugin (`:web-dist` without) |
| `./scripts/buck run //crates/block-app:web-serve` | the web bundle and `be-server`, on http://127.0.0.1:8080 |
| `./scripts/buck run //crates/block-app:android -- --install` | the APK, signed with this machine's key, installed and started (`build :android-dist` is CI's, signed on a worker with CI's key) |
| `./scripts/buck run //crates/beui:demo-example` | a crate example; every example is `<name>-example` |
| `./scripts/buck run //:rust-project` | writes `rust-project.json` for rust-analyzer |
| `./scripts/buck run //:lock-sysroot` | re-resolves `buck/sysroot/packages.bzl` |

`--target-platforms root//buck/platforms:<p>` builds for another platform, and
`<p>_release` (say `linux_x86_64_release`) is the same platform with cargo's
release profile. The profile is the `root//buck/constraints:release`
constraint rather than a buckconfig value, so one build can hold both, and every
transition keeps it. Every other build is cargo's dev profile, with
`Cargo.toml`'s `[profile.dev.package]` overrides (the optimised cranelift and
crypto crates), which `crates.bzl` carries as each crate's rustc flags. A
release build takes `-c be3.commit=SHA`, the commit the app reports; a dev
build says `unknown` whatever it is passed, so a new commit does not rebuild it.

## What `./scripts/buck` does

It runs under bash on Linux, macOS (bash 3.2) and Git Bash on Windows, and puts
these in front of the pinned buck2:

- **buck2 itself.** The release pinned in `scripts/internal/common.sh` is
  installed into `target/tools/buck2-<version>/` the first time it is missing.
  A `buck2` on `PATH` is not used, since each buck2 carries its own prelude.
- **The BuildBuddy key**: `BUILDBUDDY_API_KEY`, else `.buildbuddy-api-key` at
  the root (git ignores it), else `~/.config/be3/buildbuddy-api-key`. With
  none, a person at a terminal is asked for it and it is saved to
  `.buildbuddy-api-key`; without a terminal it fails and says what to set.
  After changing the key, restart the daemon with `./scripts/buck killall`.
- **An HTTPS proxy.** buck2's remote execution client dials BuildBuddy directly
  and never reads `HTTPS_PROXY`. When it is set, `./scripts/buck` builds
  `scripts/internal/re-relay` with Go (1.24 or newer), leaves it running on
  `127.0.0.1:18980`, and writes a `.buckconfig.local` pointing buck2 at it; the
  relay sends each call on through the proxy, over HTTP/1.1 if that is all the
  proxy speaks. It still needs a key, though a proxy that adds BuildBuddy's
  header itself accepts any value, such as `BUILDBUDDY_API_KEY=proxy-injected`.
  Its errors go to `target/re-relay.log`. A `.buckconfig.local` a person wrote
  is left alone, and the relay is not used then.
- **The generated rules.** `buck/cargo/crates.bzl` is not checked in: every
  crate's dependencies, features and targets, first- and third-party, from
  cargo's plans, with each third-party crate's checksum and size.
  `./scripts/buck` hashes its inputs and, when that changes, runs
  `buck/cargo/buckify.bxl` on a worker and copies the result into place. The action is keyed on the manifests,
  `Cargo.lock`, the paths cargo discovers targets at and `crates/buck-tools`,
  which writes `crates.bzl`, so it is shared through the cache: a few seconds
  on a fresh checkout, about a minute for the first person to change a
  dependency. The action brings `Cargo.lock` up to date with the manifests
  first, so a stale one still builds; `//:verify`'s lint writes the updated one
  back, and fails under `--check`.
- **Platforms.** Everything is built for Linux x86_64 wherever it is asked for
  (`.buckconfig`'s default target platform), so a Mac or Windows machine shares
  CI's cache. `run` is the exception: on another machine it builds for that
  machine unless the command names `--target-platforms`.
- **File modes.** A file's executable bit is part of every action that reads
  it, and a Windows checkout has none, so no file a build reads may have one:
  Windows would miss every cache entry Linux wrote. Scripts are run with `sh`,
  and `//:verify`'s lint clears the bit from anything outside `scripts/`.
- **One retry.** buck2 exits with 2 for an infrastructure error, such as
  BuildBuddy resetting a download partway, which buck2 does not retry itself.
  The wrapper runs such a command once more; the actions are cached by then.
  For `run` it builds first (`run --command-args-file`; Windows refuses
  `--emit-shell`) and retries that, since the program's own exit status is
  run's.

It also lets `test` put tests on the workers (below).

## Layout

- `.buckconfig`: cells, the execution platform, BuildBuddy. The prelude is the
  one bundled in the buck2 binary.
- `BUCK.v2`: the `//:` commands, scripts in `buck/dev`. (`BUCK.v2` rather than
  `BUCK`, which a case-insensitive filesystem cannot hold beside `buck/`.)
- `buck/tools`: every compiler and tool, downloaded and pinned by hash and size.
- `buck/toolchains`: the toolchains built from them, per target platform.
- `buck/platforms`: the execution platform (a BuildBuddy worker) and every
  target platform; `cross.bzl` lists the cross-compiled ones.
- `buck/sysroot`: the Ubuntu 24.04 packages everything is compiled against.
- `buck/cargo`: the BXL that writes `crates.bzl` and the macros that read it:
  `defs.bzl` for workspace crates, `third_party.bzl` for the rest.
- `crates/buck-tools`: the build's own helpers (the `crates.bzl` generator, the
  sysroot resolver, the APK packer, clippy's fixer and `//:rust-project`).
- `buck/wasm`: editors, plugin tests and the rules that build wasm modules.
- `buck/app`, `buck/android`: how the app, the web bundle and the APK are
  laid out.
- `third-party/rust`: every third-party crate as `<name>-<version>`, and
  `fixups.bzl`, what cargo's plans cannot say about a crate: environment for
  its build script, extra rustc flags, a file to overlay.
- `crates/<crate>/BUCK`: one per crate, written by hand.

## Adding things

**A dependency**: `cargo add` (or edit `Cargo.toml`). The next `./scripts/buck`
regenerates the rules. A build script runs as it does under cargo, and the
libraries it links or compiles reach the link; one that needs something from
the build, such as a sysroot's pkg-config, gets it from a fixup.

**A crate**: add it to the workspace, and write a `BUCK` beside its
`Cargo.toml`:

```
load("@root//buck/cargo:defs.bzl", "cargo_binary", "cargo_library", "cargo_test")

cargo_library()

cargo_test()

cargo_binary()
```

Dependencies, features, edition and roots come from `crates.bzl`. The library
is named after the package, its tests are `:test`, a binary named like the
package is `<name>-bin`, and an example is `cargo_example("<name>")`. What
`Cargo.toml` cannot say is passed as arguments: `srcs` for non-Rust files the
crate reads at compile time (default `src/**/*.rs`), `env`, `extra_deps`, or any
rule attribute such as a test's `remote_execution = "disabled"`.

**An editor**: `crates/editors/<name>/BUCK` is one `editor(name, module)` call
from `buck/wasm/defs.bzl`, which makes the guest cdylib, its `:module`, its
`:manifest` and its wasm `:test`. The app picks up every editor by itself.

**A system library**: a line in `buck/sysroot/BUCK`, then
`./scripts/buck run //:lock-sysroot`. A `-sys` crate finds it through the
fixup `_PKG_CONFIG`, which answers its build script's pkg-config from the
sysroot.

## Tests

Most tests run on the workers, including beui's and be-compositor's, which draw
through lavapipe from `buck/sysroot:amd64-test`. Two kinds stay local:

- **Plugin tests** read and write the accepted paintings in `snapshots/`.
  They are labelled `plugin`, which is how `//:verify`'s `--tests` and
  `--plugin-tests` split. Each is the crate's tests compiled to wasm and run by
  `plugin-test-runner`, the host `block-app` runs a plugin in, so they paint
  with the FreeType and HarfBuzz the plugin ships.
- `block-plugin-api`'s test that walks `crates/editors`.

Arguments after `--` go to the test executor: `--env NAME=VALUE` sets a
variable, `--test-arg NAME` runs one test. buck2 runs a binary's tests on
threads, like `cargo test`, so tests that touch process-wide state must take
turns.

## Platforms

| Platform | Built against | Linked with |
|---|---|---|
| Linux x86_64 (default) | `buck/sysroot:amd64` | lld |
| `linux_arm64` | `buck/sysroot:arm64` | lld |
| `macos_arm64`, `macos_x86_64` | MacOSX15.5 SDK (`buck/tools:macos-sdk`) | `ld64.lld` |
| `windows_arm64`, `windows_x86_64` | MSVC runtime and Windows SDK via xwin | `lld-link`, `llvm-lib` |
| `android_arm64` | NDK r29, API 26 | lld |
| `wasi` | the WASI sysroot | rust-lld (block-app on the web) |
| `wasi_guest` | the WASI sysroot | rust-lld (the plugins) |
| `wasm32` | nothing | rust-lld (the games, the gpu shim) |

All of them build on the same Linux workers. CI builds `//buck/ci:everything`
and `//crates/...` in one command: every platform's release app and dev app,
the plugins, the web bundle and the APKs (`buck/ci/BUCK`). Apple's SDK licence allows it on Apple hardware only, so release builds
for macOS move to a Mac or Asahi worker before anything ships; xwin accepts
Microsoft's Build Tools licence.

`wasi` and `wasi_guest` are the same triple, but the app links a real wgpu
backend and a plugin only wgpu's `custom` one, and the two cannot be unified.
A `guest` constraint (`buck/constraints`) tells them apart, and `crates.bzl`
has a plan for each.

A native target depends on a wasm one through a transition in
`buck/wasm/defs.bzl`: a game's test says
`env = {"GAME_WASM": "$(location :module)"}`.

## Shipping

- `:app` stages the executable as `block-app`, PDFium, and every editor's
  manifest (`<id>.plugin.json`), module and `.cwasm`, precompiled in an action
  per module for the platform the app is built for, by
  `//crates/plugin-test-runner:precompiler`, which is always the Linux x86_64
  build the workers can run.
- `:dist` is what CI ships per platform, and `:plugins` the plugins once for all.
  `//buck/ci:artifacts` is all of what CI uploads, in one directory.
- `:web` is block-app for `wasi` and `block-gpu-shim` for `wasm32`, each run
  through `wasm-bindgen` (`buck/cargo:wasm-bindgen`, pinned to `Cargo.lock`'s
  version), with the page and shims from `crates/block-app/web` and a
  `plugins.json` the browser finds the plugins through. `:web-serve` runs Caddy
  (`buck/tools:caddy`) with `web/Caddyfile`, and `-- --domain DOMAIN` serves
  that domain over https, which is how the app is deployed.
- The APK is assembled on a worker without Gradle (`buck-tools apk`):
  aapt2, javac and d8, block-app's `[cdylib]` and `libc++_shared.so`, the
  plugins precompiled for arm64 (only the `.cwasm`s), and `zipalign -P 16`. The Java is
  the app's own and beui's (`//crates/beui:android-java`), against the
  platform alone: the APK carries no libraries. `:android` signs it
  locally with `target/android-debug.keystore`, made on first use.
  `:android-dist` signs on a worker with CI's keystore, which BuildBuddy keeps
  as the secret `ANDROID_DEBUG_KEYSTORE_BASE64` and passes only to actions on
  the `android_signing` execution platform, with its own application id and
  label (`com.be3.block.ci`, `Block (CI)`) so it installs beside a local build.
  After changing the secret, bump `key_version` in `crates/block-app/BUCK`.
  The host's wasmtime has cranelift's arm64 backend for the arm64 precompiles
  (a fixup on `cranelift-codegen`).
- `crates/be-launcher` is an APK too. It runs, in block-app's own
  `MainActivity`, the builds CI uploads to be3-ci: `:android-run`, the release
  library and the plugins' `.cwasm`s (the `publish-android` job in ci.yml
  says how they are stored). A build runs only in a launcher with its shell
  hash (`:android-shell`, over block-app's Java, beui's Java and the
  manifest), so a change to those needs a new launcher, and a permission added to
  block-app's manifest goes in the launcher's too.
- The macOS builds are an executable and its libraries; the `.app` bundle
  comes with distribution.

## rust-analyzer

`./scripts/buck run //:rust-project` writes `rust-project.json` (git ignores
it), which rust-analyzer prefers over `Cargo.toml`. It covers the host and, as
`wasm32-wasip1-threads`, the plugins, takes the standard library and proc-macro
server from the pinned toolchain, and checks on save through buck2. Run it
again after adding a crate or dependency, and use the rust-analyzer an editor
extension ships: the toolchain's panics on this workspace.

## Things that are not obvious

- **Features.** Every crate has the features cargo's plan for the platform
  gives it, and the plans are `cargo test` for the host and each cross
  platform, so what a dev-dependency turns on is on for the library too, as
  under cargo. What cargo builds for the machine running the build - proc
  macros, build scripts and their dependencies - is one build here, the
  `linux-x86_64` one, with the union of what every plan asks of it.
- **The compiler stays stable.** `nightly_features = False` in
  `buck/toolchains/BUCK`: otherwise the prelude sets `RUSTC_BOOTSTRAP=1`, and
  `cfg(target_feature = "atomics")` on `wasm32-wasip1-threads` then answers
  differently from cargo, which broke wgpu's `Send`/`Sync` checks.
- **The key.** A wrong or missing key looks like a build with no cache hits,
  not an error. `buck2 log show | grep -i 'invalid api key'` finds it.
- **Downloads.** Every `http_archive` has `size_bytes` as well as `sha256`;
  without it buck2 sends a HEAD request per download on every new daemon.
- **Clippy** runs through `buck/dev/workspace.bxl` over every target's
  `[clippy.json]` in every configuration, including the wasm ones, which is why
  wasm-only code is linted. `clippy.toml` is empty but must exist.
- **Build metadata.** BuildBuddy reads it from Bazel's Build Event Stream,
  which buck2 does not implement, so its invocations carry no branch or commit.
