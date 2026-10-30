# buck2

buck2 builds, lints and tests the workspace, and every action runs on a
remote execution server, Namespace's unless `BE3_BUILD_SERVER` picks another
(guides/build_server.md). Every command is a buck2 target started through
`./scripts/buck`, except `./scripts/verify` and `./scripts/ci`, which are
commands of their own so that each is one buck2 command. Every script is
`crates/bootstrap` (guides/bootstrap.md). cargo builds nothing; `Cargo.toml` is still the one place a
dependency is declared, and buck2 reads it through cargo's own plans.

Every command and target is in guides/common_commands.md.

## Profiles and build info

`--target-platforms root//buck/platforms:<p>` builds for another platform, and
`-m release` with cargo's release profile, on whichever platform it is. The
profile is the `root//buck/constraints:release` constraint, added by a
configuration modifier (the root `PACKAGE`) rather than a buckconfig value, so
one build can hold both, and every transition keeps it. Every other build is cargo's dev profile, with
`Cargo.toml`'s `[profile.dev.package]` overrides (the optimised cranelift and
crypto crates), which `crates.bzl` carries as each crate's rustc flags.

Which build a program is lives in a file beside it, never in its code:
`//buck/app:build-info.json` is the JSON file `buck/app/build-info.json`, and every bundle
(`:native`, `:native-dist`, `:web`, `:web-dist`, the APK's assets, be-cli's
`:native`, be-server-bin's `:dist`)
carries it. A client and a server refuse each other unless their ids match, a
program that finds no id refuses to connect or serve, and the app shows the id
as its build. The metadata contains `id` (the compatibility identity) and `app_id` (`BlockDev`
by default, `Block` for deploy). The app reads `app_id` at startup for its window
and data directory, so deployment identity changes only packaging. `BE3_APP_ID`
selects it when generating the file. `./scripts/deploy` also includes an optional
`default_server` descriptor (public name, API and authentication URLs, and whether
native clients need a gateway token); ordinary local and CI builds omit it. The id is the hash of the tree, not of a commit, so two commits
of the same tree (a pull request's head and the merge queue's commit of it)
share every cached bundle, APK and app test run. Since only the bundles read the
file, a new id rebuilds nothing else.

## What `./scripts/buck` does

It is `bootstrap buck` (guides/bootstrap.md), on Linux, macOS and Windows (from
Git Bash, whose `sh` runs the stub), and puts these in front of the pinned
buck2:

- **buck2 itself.** The release pinned in `scripts/internal/pins` is
  installed into `target/tools/buck2-<version>/` the first time it is missing.
  A `buck2` on `PATH` is not used, since each buck2 carries its own prelude.
- **The build server.** `BE3_BUILD_SERVER` picks it (or the prompt on the
  first run), and its hosts and key go
  into a generated `.buckconfig.local`, with the server's name as
  `be3.build_server`, which picks the workers' image. For Namespace, the
  default, the hosts are asked of the build helper at `api.be3.pfg.pw`, which
  wakes the remote execution cluster (guides/build_server.md says when it
  asks, and where each server's key comes from).
- **An HTTPS proxy.** buck2's remote execution client dials the server's hosts
  directly and never reads `HTTPS_PROXY`. When it is set, `./scripts/buck` builds
  `scripts/internal/re-relay` with Go (1.24 or newer), leaves one running for the executor and one for the storage, on two
  ports of `127.0.0.1` picked from the checkout's path, so that two checkouts on
  one machine almost never share one, and points buck2 at them in the same `.buckconfig.local`; each relay sends
  its calls on through the proxy, over HTTP/1.1 if that is all the proxy
  speaks. Their errors go to `target/re-relay.log`. A `.buckconfig.local` a
  person wrote is left alone, and the relays are not used then.
- **Platforms.** Everything is built for Linux x86_64 wherever it is asked for
  (`.buckconfig`'s default target platform), so a Mac or Windows machine shares
  CI's cache. `run` is the exception: on another machine it builds for that
  machine unless the command names `--target-platforms`.
- **File modes.** A file's executable bit is part of every action that reads
  it, and a Windows checkout has none, so no file a build reads may have one:
  Windows would miss every cache entry Linux wrote. Scripts are run with `sh`,
  and `./scripts/verify`'s lint clears the bit from anything outside `scripts/`.
- **The build id.** Every command that builds or queries first writes the
  working tree's hash into the `id` field of `buck/app/build-info.json`, which git ignores, if it
  changed. The `app_id` field comes from `BE3_APP_ID` (`BlockDev` by default). The hash is taken through a throwaway index, so untracked files
  count and the same tree always gives the same id. As a source file, a new id
  invalidates only what reads it. `BE3_BUILD_ID` overrides it.
- **Retries.** buck2 exits with 2 for an infrastructure error, such as a
  connection to the build server resetting partway through a download, which buck2 does not retry itself.
  The wrapper runs such a command again, three times in all (`BE3_BUCK_ATTEMPTS`); the finished actions are cached by then.
  For `run` it builds first (`run --command-args-file`; Windows refuses
  `--emit-shell`) and retries that, since the program's own exit status is
  run's.

It also lets `test` put tests on the workers (below).

## Layout

- `.buckconfig`: cells and the execution platform. The prelude is the
  one bundled in the buck2 binary.
- `BUCK.v2`: the `//:` commands, scripts in `buck/dev`. (`BUCK.v2` rather than
  `BUCK`, which a case-insensitive filesystem cannot hold beside `buck/`.)
- `buck/tools`: every compiler and tool, downloaded and pinned by hash and size.
- `buck/toolchains`: the toolchains built from them, per target platform.
- `buck/platforms`: the execution platform (a build server's worker) and every
  target platform; `cross.bzl` lists the cross-compiled ones and the plans
  `crates.bzl` keys them by, and everything else that lists platforms reads it.
- `buck/sysroot`: the Ubuntu 24.04 packages everything is compiled against.
- `buck/cargo`: the BXL that writes `crates.bzl` and the macros that read it:
  `defs.bzl` for workspace crates, `third_party.bzl` for the rest.
  `crates.bzl` is generated and checked in, like a lockfile: every crate's
  dependencies, features and targets, first- and third-party, from cargo's
  plans, with each third-party crate's checksum and size.
  `./scripts/buck run //:buckify` runs `buckify.bxl` on a worker and copies the
  result into place, with `Cargo.lock` brought up to date with the manifests;
  `./scripts/verify` builds the same action and copies it too, and under
  `--check` fails if either is out of date; its build uses the checked-in
  copies, so a stale one is fixed by the run after. The action is keyed on the manifests, `Cargo.lock`, the
  paths cargo discovers targets at and `crates/buck-tools`, so it is shared
  through the cache.
- `crates/buck-tools`: the build's own helpers (the `crates.bzl` generator, the
  sysroot resolver, the APK packer and `//:rust-project`).
- `crates/bootstrap`: what every script runs, a prebuilt release
  (guides/bootstrap.md).
- `crates/verify`, `crates/local-tools`: what runs on this machine after a
  buck2 command, built for it in that command: `verify` for `./scripts/verify`,
  and local-tools' `ci`, `deploy` and `fuzz`.
- `buck/wasm`: editors, plugin tests and the rules that build wasm modules.
- `buck/app`, `buck/android`: how the app, the web bundle and the APK are
  laid out.
- `third-party/rust`: every third-party crate as `<name>-<version>`, and
  `fixups.bzl`, what cargo's plans cannot say about a crate: environment for
  its build script, extra rustc flags, a file to overlay.
- `crates/<crate>/BUCK`: one per crate, written by hand.

## Adding things

**Target names** are kebab-case, and a role has one name in every package:
`:bin` a crate's binary, `:test` its tests and `:test-run` the action that
runs them, `:example-<name>` and `:fuzz-<name>` its examples and fuzz targets,
`:wasm` a guest library and `:module` its module. An app's targets are named by
platform, `:native`, `:web` and `:android`, each the bundle that `buck2 run`
runs, serves or installs here, with `-dist` the build CI publishes, and `:dev`
the app headless for an agent to drive.

**A dependency**: `cargo add` (or edit `Cargo.toml`), then
`./scripts/buck run //:buckify` to regenerate the rules. That includes a
dev-dependency on another crate of the workspace: until it runs, the crate's
`:test` cannot see it. A build script runs
as it does under cargo, and the libraries it links or compiles reach the link;
one that needs something from the build, such as a sysroot's pkg-config, gets
it from a fixup.

**A crate**: add it to the workspace, write a `BUCK` beside its
`Cargo.toml`, and run `./scripts/buck run //:buckify`:

```
load("@root//buck/cargo:defs.bzl", "cargo_binary", "cargo_library", "cargo_test")

cargo_library()

cargo_test()

cargo_binary()
```

Dependencies, features, edition and roots come from `crates.bzl`. The library
is named after the package, its tests are `:test`, its binary `:bin`, and an
example is `cargo_example("<name>")`, which makes `:example-<name>`. What
`Cargo.toml` cannot say is passed as arguments: `env`, `extra_deps`, or any
rule attribute such as a test's `remote_execution = "disabled"`.

Sources are inferred, including non-Rust files read at compile time. The common
library, binary, example and test rules also carry the manifest at
`CARGO_MANIFEST_DIR/Cargo.toml`, so procedural macros can discover dependency
names during remote compilation. Other sources sit at their path in the package,
not under `CARGO_MANIFEST_DIR`, so a file a procedural macro reads from there is
passed as `resources` to `cargo_library` and `cargo_test`, as block-type-api's
WIT is (`resources = glob(["wit/*.wit"])`). Libraries
use `src/**` except `src/bin/**`, `tests.rs`, `tests/**`, `main_tests.rs` and
`main_tests/**` at any depth. Tests use all of `src/**` except `src/bin/**`.
Keep test-only assets under a `tests/` directory. Binaries and examples in a
crate with a library use only their root file, or the root's whole directory
when it is a `main.rs` in a subdirectory below `src/` or `examples/`.
Binary-only crates use the library's source rules.
Roots come from `crates.bzl`, including block-app's `src/main.rs` library and
`src/desktop.rs` binary. The wasm SDK and editors use the same library rules,
and plugin tests use the test rules; editors also include `manifest.json`.

**An editor**: `crates/editors/<name>/BUCK` is one `editor(module)` call
from `buck/wasm/defs.bzl`, which makes the guest cdylib `:wasm`, linked against the
plugin SDK (guides/plugin_sdk.md), its `:module`, its `:manifest` and its wasm
`:test`. The app picks up every editor by itself.

**A fuzz target**: a file `fuzz/<name>.rs` in a crate whose `BUCK` calls
`cargo_fuzz()` (with `extra_features` for the library's features it needs),
which is a `#![no_main]` crate defining `LLVMFuzzerTestOneInput` over the
library (`crates/sequence/fuzz`). It becomes `:fuzz-<name>`, and `//:fuzz`
finds it by itself (`buck/dev/fuzz.bxl`). It is built with rustc's sancov
instrumentation and linked with LLVM's libFuzzer, on the stable compiler and
without a sanitizer, for Linux x86_64 only, and `:fuzz-<name>-test` fuzzes for
a moment from a fixed seed. Its corpus and crashes are kept in
`target/fuzz/<crate>/<name>`, so it picks up where it was when it is started
again.

**A system library**: a line in `buck/sysroot/BUCK`, then
`./scripts/buck run //:lock-sysroot`. A `-sys` crate finds it through the
fixup `_PKG_CONFIG`, which answers its build script's pkg-config from the
sysroot.

## Tests

Most tests run on the workers, including beui-renderer-wgpu's, be-dmabuf's, be-wayland's and
beui-platform-drm's, which draw
through lavapipe from `buck/sysroot:amd64-test` (beui-layer-xr's from
`buck/sysroot:amd64-xr`, the same set with Monado's OpenXR runtime and
service, which its test runs headless); a test whose `env` names
`LD_LIBRARY_PATH` is built as a harness binary and run through
`library_path_test`, since the workers replace that variable
(guides/build_server.md). buck2 runs a test again every
time, so `cargo_test` also makes `:test-run`, an action that runs the test on a
worker and passes when it does, which is cached like any other; `./scripts/verify`
builds those instead. A test with `remote_execution = "disabled"` gets a
`:test-run` that runs here. compile_fail's tests (below) have one too.

`./scripts/test` runs tests on workers too, through `buck/dev/test.bxl`: an
action for each `:test` that runs its command with the name to filter by and keeps the
test's output, its exit status and the paintings it accepted, which is all that
comes back; `buck2 test` would download the test binary and everything it reads,
hundreds of megabytes for block-app's. A test's `ExternalRunnerTestInfo` is
the command, unless it has a `RemoteTestInfo` (`buck/cargo/defs.bzl`), as the
tests that paint do, naming the accepted paintings and the worker's lavapipe.

**Plugin tests** read and write the accepted paintings in their crate's `snapshots/`, and only those, so a painting one crate accepts leaves every other crate's tests cached. Each is the crate's tests compiled to wasm and run by `plugin-test-runner`,
the host `block-app` runs a plugin in, so they paint with the FreeType and
HarfBuzz the plugin ships. `./scripts/test --update` copies what a crate's
`:test` accepted on a worker into the crate's `snapshots/`. `./scripts/verify` builds each
crate's `:test-run`, an action that runs them on a worker and is cached like
any other: it outputs the paintings that changed and the names of the ones it
compared, and `./scripts/verify` copies the first into the crate's `snapshots/` and
deletes the paintings no test named.

**Native tests that paint**, `cargo_test(paints = True, run_env = ...)`, are
treated the same way: `:test` is what `./scripts/test` runs and `:test-run` is the action
`./scripts/verify` reads paintings from. `run_env` is the environment the test
runs with, kept out of its compile. crates/app-tests runs block-app's `:native` headless, so it runs on Linux x86_64 only.
It also passes `run_apart = True`, which makes a `:test-run-<file>` for each file under
`src/tests/` in place of one `:test-run`: a failed action keeps no outputs, so a failing test
then loses only its own paintings, not every one the crate accepts.
crates/be-portal-ui captures its SSR portal as text browser paintings without a browser.

**Browser tests** (`web_drive_test` in `buck/app/drive.bzl`) are the one kind that runs on this
machine rather than a worker, since the workers have no browser: their `:<name>-run` is a
local-only action in ./scripts/verify, and ./scripts/test runs a test labelled `local` here
(buck/dev/test.bxl). They need Node, its `playwright` package and a Chrome or Chromium
(guides/testing_a_gui.md, 7). A crate on the HTML core builds its browser module with
`wasi_html_module` (buck/wasm/defs.bzl), as `wasi_app_module` builds block-app's.

Each compile-fail suite keeps a `Cargo.toml` listing its dependencies. The rule
maps it into the compiler sandbox for automatic macro support-crate discovery.

`compile_fail/` holds code that must not compile: `compile_fail(name, deps)`
in its `BUCK` makes `<name>/lib.rs` a crate and `:<name>-test`, which reads
the crate's `[diag.json]` and passes when every line marked
`//~ ERROR <part of the message>` has that error and no other line has one.
It is rustc's check pass once for all the cases, with nothing linked. The
crates sit outside `crates/` because `build //crates/...`, `//:check` and
`./scripts/verify` would fail on them, and the autofixes would delete the markers.

Arguments after `--` go to the test executor: `--env NAME=VALUE` sets a
variable, `--test-arg=NAME` runs one test. Write it with `=`: the spaced
`--test-arg NAME` takes every argument after it as its value, a second
`--test-arg` included, and the test binary then fails on that. buck2 runs a binary's tests on
threads, like `cargo test`, so tests that touch process-wide state must take
turns.

## Platforms

| Platform | Built against | Linked with |
|---|---|---|
| Linux x86_64 (default) | `buck/sysroot:amd64` | lld |
| `linux_arm64` | `buck/sysroot:arm64` | lld |
| `macos_arm64`, `macos_x86_64` | MacOSX15.5 SDK (`buck/tools:macos-sdk`) | `ld64.lld` |
| `windows_arm64`, `windows_x86_64` | MSVC runtime and Windows SDK via xwin | `lld-link`, `llvm-lib` |
| `android_arm64` | NDK r29, API 24 | lld |
| `wasi` | the WASI sysroot | rust-lld (block-app on the web) |
| `wasi_guest` | the WASI sysroot | rust-lld through crates/wasm-link (the plugins and their SDK) |
| `wasm32` | nothing | rust-lld (the games, the gpu shim) |
| `wasi_html` | the WASI sysroot | rust-lld (crates on beui's HTML core, in the browser) |
| `linux_x86_64_html` | `buck/sysroot:amd64` | lld (crates on beui's HTML core, on the host: server rendering, string tests) |

All of them build on the same Linux workers. `./scripts/verify` also runs
rustc's check pass over beui for Windows x86_64 (`_CROSS_CHECKS` in
`buck/dev/verify.bxl`), where beui-layer-xr's first headset is, and over every
library planned for beui's HTML core on its two platforms, in its one buck2
command. CI ships nothing built for a
device: the build helper (guides/build_server.md) builds the Android app and
launcher (`:android-dist`) and the desktop app for any platform when
be-launcher asks for a commit; releases are built by hand. Apple's SDK licence allows it on Apple hardware only, so release builds
for macOS move to a Mac or Asahi worker before anything ships; xwin accepts
Microsoft's Build Tools licence.

`wasi` and `wasi_guest` are the same triple, but the app links a real wgpu
backend and a plugin only wgpu's `custom` one, and the two cannot be unified.
A `guest` constraint (`buck/constraints`) tells them apart, and `crates.bzl`
has a plan for each.

`wasi_html` and `linux_x86_64_html` are `wasi` and the host with the `html`
constraint (`buck/constraints`), which picks beui's HTML core
(plans/html_core.md). Their plans, `wasi-html` and `linux-x86_64-html`, are
cargo's for the crates `html_packages` in `buck/cargo/buckify.sh` names (beui and the crates under it, and the crates built only on the HTML core; `browser_packages` adds the ones built only for the browser, which only the `wasi-html` plan takes), run
with `RUSTFLAGS="--cfg beui_html"`; cargo evaluates
`[target.'cfg(beui_html)'.dependencies]` against it, so a manifest says what
each core links (beui-view takes `beui-core` only under
`cfg(not(beui_html))`), and `defs.bzl` passes `--cfg beui_html` to every
first-party rustc on those platforms. `buck/cargo/html.bzl` lists the
libraries in those plans, and `./scripts/verify` checks each on its platform; a test or program that runs on the host's HTML core names its
platform as `default_target_platform`, as beui-view's `:html-test` does, and a
browser module transitions its library to `wasi_html` with `wasi_html_module`
(`buck/wasm/defs.bzl`). Code chooses a core with
`if CORE == Core::Html` (`beui::CORE`); `#[cfg(beui_html)]` belongs only in
beui-view, beui and beui-core-html, which fix-rust-source enforces.

A native target depends on a wasm one through a transition in
`buck/wasm/defs.bzl`: a game's test says
`env = {"GAME_WASM": "$(location :module)"}`.

## Shipping

- `:native` stages the executable as `block-app` and every editor's
  manifest (`<id>.plugin.json`), data (`data/<id>/`), module and `.cwasm`, and
  the plugin SDK as `sdk-<SHA-256>.wasm` with its `.cwasm`, precompiled in an action
  per module for the platform the app is built for, by
  `//crates/plugin-test-runner:precompiler`, which is always the Linux x86_64
  build the workers can run. `plugins.json` lists the manifests: every
  platform finds its plugins through it, beside the executable, in the APK's
  assets, or at the bundle root.
- `:native-dist` is a platform's release, `:plugins` the plugins once for
  all, and `//crates/be-server-bin:dist` the server with its build id.
- `:web` is block-app for `wasi` and `block-gpu-shim` for `wasm32`, each run
  through `wasm-bindgen` (`buck/cargo:wasm-bindgen`, pinned to `Cargo.lock`'s
  version), with the page and shims from `crates/block-app/web`. `buck2 run` on it runs Caddy
  (`buck/tools:caddy`) with `web/Caddyfile` and be-server behind it, and `-- --domain DOMAIN` serves
  that domain over HTTPS. Production deployment uses Bunny (guides/hosting.md).
- The APK is assembled on a worker without Gradle (`buck-tools apk`):
  aapt2, javac and d8, block-app's `[cdylib]` and `libc++_shared.so`, the
  plugins precompiled for arm64 (only the `.cwasm`s), and `zipalign -P 16`. The Java is
  the app's own and beui's (`//crates/beui-platform-android:android-java`), against the
  platform alone: the APK carries no libraries. Every Android target builds
  an aligned unsigned template without build metadata. `buck2 run` appends
  metadata and signs on the requesting machine with `local-tools apk-sign`. `:android` uses `target/android-debug.keystore`, made on first use.
  `:android-dist` has its own identity (`com.be3.block.ci`, `Block (CI)`), and
  the build helper finalizes both APKs with CI's key. Deploy finalizes `:android-release` locally before upload.
  Only signed, finalized APKs can be installed; `buck build --out` returns a
  template. The minimum Android API and native toolchain API are 24, and
  signing uses v2 alone. Keys never enter action inputs (guides/running/android.md).
  The host's wasmtime has cranelift's arm64 backend for the arm64 precompiles
  (a fixup on `cranelift-codegen`).
- `crates/be-launcher` lists the pull requests and runs any of them, or main,
  on Android and the desktop alike: it asks the build helper to build the
  commit (`android`, `launcher-android`, `native-PLATFORM`,
  `beui-demo-PLATFORM`), streams the build's log into its Log pane, downloads
  from be3-ci only the files of the manifest it does not already have, by
  their SHA-256, then installs the APK or starts the app. It needs the API
  gateway's key, which it asks for and keeps (`~/.config/be3/gateway-authorization`
  on the desktop, its own files on Android); `BE3_AUTHORIZATION` overrides it.
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
  under cargo. One plan covers every crate built for its platform, so
  features merge across apps: the `wasi` plan is block-app's, beui-web-demo's
  and paint-review-web's at once, and a feature one of them turns on is on
  for all three. What has to differ between builds for one triple needs a
  platform and plan of its own, told apart by a constraint, as `wasi_guest`
  and `wasi_html` are. What cargo builds for the machine running the build - proc
  macros, build scripts and their dependencies - is one build here, the
  `linux-x86_64` one, with the union of what every plan asks of it.
- **Pipelining.** `nightly_features = True` in `buck/toolchains/BUCK`, so a
  crate's dependents compile against a `-Zno-codegen` metadata compile of it
  rather than wait for its codegen; the prelude then sets `RUSTC_BOOTSTRAP=1`
  for every rustc. The metadata compile and the real one must agree on the
  crate's hash, so a crate must not take in the absolute path of the directory
  its compile runs in, which is different for every action: not by
  `env!("CARGO_MANIFEST_DIR")`, nor by a macro that writes an `include_bytes!`
  of an absolute path. A dependent's link otherwise fails with E0460, "found
  possibly newer version of crate". `third-party/rust/fixups.bzl` overlays the
  third-party crates that did (witx, wasmtime's `bindgen!`).
- **Plugins compile at opt-level 0** outside a release build, the workspace
  crates under them for wasm at 1 (`_wasm_dev` in `buck/cargo/defs.bzl`), and
  only third-party crates at the plugin profile's 2, which their cache hides
  (`editor()` and `plugin_tests()` in `buck/wasm/defs.bzl`); test modules are
  precompiled with Cranelift's optimizations off (`--quick`). A change under
  beui recompiles all of them, and at 2 that was a third of the CPU a change to
  `geometry` cost `./scripts/verify`.
- **Parallel frontend.** Every workspace rustc runs with `-Zthreads=8`
  (`_rustc_flags` in `buck/cargo/defs.bzl`): the crates under beui are a chain
  each of whose typechecks waits on the one before, which this shortens.
- **The key.** A server answers a call with a wrong or missing key or token
  with `UNAUTHENTICATED`, which buck2 reports as failing to reach it.
- **Downloads.** Every `http_archive` has `size_bytes` as well as `sha256`;
  without it buck2 sends a HEAD request per download on every new daemon.
- **`./scripts/verify`** is one `buck2 bxl` of `buck/dev/verify.bxl`, which
  builds everything with `--keep-going` and prints what is read after: the
  generated files, the paintings, each crate's fixes, and `crates/verify`,
  built for this machine in the release profile, which the script then runs
  to read the rest and write it into the checkout. It depends on nothing but
  sha2, since every run downloads it; for CI's Android builds and paint
  previews it runs `crates/local-tools ci`, built only then. Anything else
  that runs on this machine after a buck2 command belongs in
  `crates/local-tools`, as a subcommand, rather than in shell or a crate of
  its own, so it works the same on Linux, macOS and Windows and builds once
  (what runs before one is `crates/bootstrap`): `ci`, `deploy`
  (`./scripts/deploy`) and `fuzz` (a fuzz target's runner). The autofixes are actions (`buck/dev/fix.sh`), one
  a crate: fix-rust-source, then rustfmt, on a copy of the
  crate's sources, output as the files that changed. A fix that makes another
  possible shows up on the next run.
- **Lints** are rustc's, and a warning fails the compile that finds it, so
  `./scripts/verify` sees them only in what its tests compile, `linux_x86_64`
  and `wasi_guest`, and in what it checks: beui for Windows and the HTML
  core's libraries. It neither typechecks other configurations (Android, the
  web's wasm32, host wasi, release copies) nor runs clippy: each was a second
  compile of every target, which doubled what a change cost. `//:check`
  typechecks less: only `linux_x86_64` and `wasi_guest`
  (`buck/dev/workspace.bxl`), so an agent's check is a third of the actions it
  was. Code behind `cfg(windows)` or `cfg(target_os = "macos")` outside beui is
  first compiled by `./scripts/deploy`.
- **Clippy** is still in the toolchain:
  `./scripts/buck bxl //buck/dev/workspace.bxl:subtarget -- --subtarget clippy.txt`
  runs it over every target and prints where each target's findings are.
  `clippy.toml` is empty but must exist for that.
