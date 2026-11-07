# buck2

buck2 builds, lints and tests the workspace, and every action runs on a remote execution
server (guides/build_server.md). Every script is `crates/be-tool-bootstrap`
(guides/bootstrap.md); `./scripts/verify` and `./scripts/ci` are one buck2 command each.
cargo builds nothing: `Cargo.toml` is the one place a dependency is declared, and buck2 reads it
through cargo's own plans. Every command and target is in guides/common_commands.md.

## Layout

- `.buckconfig`: cells and the execution platform; the prelude is the one bundled in buck2.
- `BUCK.v2`: the `//:` commands, with their scripts in `buck/dev` (`BUCK.v2` because a
  case-insensitive filesystem cannot hold `BUCK` beside `buck/`).
- `buck/tools`: every compiler and tool, downloaded and pinned by hash and size;
  `buck/toolchains`: the toolchains built from them.
- `buck/platforms`: the worker's execution platform and every target platform; `cross.bzl`
  lists the cross-compiled ones, and everything else that lists platforms reads it.
- `buck/sysroot`: the Ubuntu 24.04 packages everything is compiled against.
- `buck/cargo`: `crates.bzl`, generated and checked in like a lockfile (every crate's
  dependencies, features and targets from cargo's plans, and every Cargo.lock crate's download),
  and the macros that read it: `defs.bzl`
  for workspace crates, `third_party.bzl` for the rest.
- `buck/wasm`: apps, app tests and the wasm module rules. `buck/program`, `buck/android`: the
  native bundles, the web bundle and the APK.
- `third-party/rust`: every third-party crate, and `fixups.bzl` for what cargo's plans cannot say
  (build script environment, rustc flags, file overlays).
- `crates/<crate>/BUCK`: one per crate, by hand.
- `crates/be-tool-buck`: the build's own helpers (the `crates.bzl` generator, sysroot resolver,
  APK packer, `//:rust-project`). `crates/be-tool-verify` and `crates/be-tool-local` run on this
  machine after a buck2 command (guides/bootstrap.md says which code goes where).

The `.bzl` files carry comments on each rule; read them before changing one.

## Profiles, platforms and build info

`-m release` builds with cargo's release profile; it is the `buck/constraints:release`
constraint, added by a modifier in the root `PACKAGE`, so one build can hold both profiles.
Everything else is cargo's dev profile with `Cargo.toml`'s `[profile.dev.package]` overrides.
`--target-platforms root//buck/platforms:<p>` builds for another platform. Everything is built
for Linux x86_64 unless asked, so any machine shares CI's cache; `run` on another machine builds
for that machine.

| Platform | Built against | Linked with |
|---|---|---|
| Linux x86_64 (default) | `buck/sysroot:amd64` | lld |
| `linux_arm64` | `buck/sysroot:arm64` | lld |
| `macos_arm64`, `macos_x86_64` | MacOSX15.5 SDK | `ld64.lld` |
| `windows_arm64`, `windows_x86_64` | MSVC runtime and Windows SDK via xwin | `lld-link` |
| `android_arm64` | NDK r29, API 24 | lld |
| `wasi` | the WASI sysroot | rust-lld (be-host on the web) |
| `wasi_guest` | the WASI sysroot | crates/be-wasm-link (apps and their libraries) |
| `wasm32` | nothing | rust-lld (games, the gpu shim) |
| `wasi_html`, `linux_x86_64_html` | as `wasi`, Linux x86_64 | crates on beui's HTML core |

All build on the same Linux workers. Apple's SDK licence allows it on Apple hardware only, so
macOS releases must move to a Mac worker before anything ships.

`wasi` and `wasi_guest` are the same triple: be-host links a real wgpu backend and an app only
wgpu's `custom` one, so the `guest` constraint (`buck/constraints`) tells them apart and
`crates.bzl` has a plan for each. Likewise the `html` constraint makes `wasi_html` and
`linux_x86_64_html`, whose plans are run with `RUSTFLAGS="--cfg beui_html"` over the packages
`html_packages` in `buck/cargo/buckify.sh` names (guides/beui/core.md, Cores). Code picks a core
with `if CORE == Core::Html`; be-tool-fix-rust-source rejects `#[cfg(beui_html)]` outside beui,
beui-view and beui-core-html.

`buck/program/build-info.json` (git-ignored) is carried by every bundle, so a new build id
rebuilds nothing else:

- `id` is the hash of the working tree, untracked files included, not of a commit, so a pull
  request's head and the merge queue's commit share every cached bundle and host test.
  `./scripts/buck` rewrites it before each command (`BE3_BUILD_ID` overrides). A client and a
  server refuse each other unless their ids match.
- `program_id` is `be-host-dev`, or `be-host` for the deploy (`BE3_PROGRAM_ID`); be-host takes its
  window title and data directory from it.
- `default_server`, written only by `./scripts/deploy`, is the server a build connects to.

## What `./scripts/buck` adds

- **buck2 itself**, the release pinned in `scripts/internal/pins`, in `target/tools/`. A `buck2`
  on `PATH` is not used, since each carries its own prelude.
- **The build server's** hosts and key in a generated `.buckconfig.local`
  (guides/build_server.md). A `.buckconfig.local` a person wrote is left alone.
- **An HTTPS proxy relay.** buck2's remote execution client never reads `HTTPS_PROXY`, so when it
  is set `./scripts/buck` builds `scripts/internal/re-relay` with Go (1.24+) and runs two relays
  on ports picked from the checkout's path; errors go to `target/re-relay.log`.
- **Retries.** buck2 exits 2 on an infrastructure error, such as a reset partway through a
  download, and does not retry; the wrapper runs the command three times in all
  (`BE3_BUCK_ATTEMPTS`). For `run` it retries the build, not the program.
- **Tests on the workers** for `./scripts/test` (below).

A file's executable bit is part of every action that reads it, and a Windows checkout has none,
so `./scripts/verify`'s lint clears the bit outside `scripts/`; scripts are run with `sh`.

## Adding things

**Target names** are kebab-case, one name per role: `:bin`, `:test` and `:test-run` (the action
that runs it), `:example-<name>`, `:fuzz-<name>`, `:wasm` (a guest library) and `:module`. A
bundled program (`program()` in buck/program/defs.bzl) has `:native`, `:web`, `:android`, the
`-dist` build CI publishes, and `:dev`, headless for an agent to drive.

**A dependency**: `cargo add`, then `./scripts/buck run //:buckify`, which regenerates
`crates.bzl` and `Cargo.lock` on a worker. That includes a dev-dependency on a workspace crate:
until it runs, the crate's `:test` reports it unresolved. `./scripts/verify` regenerates them
too, but builds with the checked-in copies, so a stale one is fixed by the run after.

**A crate**: add it to the workspace, write a `BUCK` beside `Cargo.toml`, run `//:buckify`:

```
load("@root//buck/cargo:defs.bzl", "cargo_crate")

cargo_crate()
```

`cargo_crate()` makes every target `Cargo.toml` has: the library and its `:test`, each `[[bin]]`
and each example; `cargo_crate(test = False)` for a library with no tests. Dependencies,
features, edition, roots and sources come from `crates.bzl` and globs (`cargo_library_sources` in
buck/cargo/defs.bzl). A crate that needs to say what `Cargo.toml` cannot (`env`, `extra_deps`,
`resources`, or any rule attribute) calls `cargo_library`, `cargo_test`, `cargo_binary` and
`cargo_example("<name>")` itself, with those as arguments. Every target in the repository is
visible to every other unless it says otherwise: the root `PACKAGE` sets the default.

**An app**: `crates/be-app-<name>/BUCK` is one `app(module)` call (buck/wasm/defs.bzl), which
makes `:wasm`, `:module`, `:manifest` and the wasm `:test`, linked against `DEFAULT_LIBRARIES`
unless it names others (guides/app_libraries.md). be-host picks up every app by itself. Its
block types are one `type_module(module)` (guides/adding_a_block.md), a game is one
`game_module()` (guides/adding_a_game.md), and any other wasm32 module `wasm32_crate()`.

**A program in the browser**: `web_library(role)` in place of `cargo_library()`, and
`web_program(name, role, module, files, serve)` for its bundle (buck/wasm/defs.bzl), where the
role is `program` for beui's native core and `html` for its HTML core. `WASM_ROLES` there is
every wasm platform a module is built for and what it is linked with; no BUCK file names those
flags or platforms itself.

**A fuzz target**: `fuzz/<name>.rs` in a crate whose BUCK calls `cargo_fuzz()`, a `#![no_main]`
crate defining `LLVMFuzzerTestOneInput` (crates/be-lib-sequence/fuzz). It becomes
`:fuzz-<name>`, `//:fuzz` finds it, and `:fuzz-<name>-test` fuzzes briefly from a fixed seed.
Its corpus is kept in `target/fuzz/<crate>/<name>`.

**A system library**: a line in `buck/sysroot/BUCK`, then `./scripts/buck run //:lock-sysroot`.
A `-sys` crate's pkg-config is answered from the sysroot by the `_PKG_CONFIG` fixup.

## Tests

buck2 runs a test again every time, so `cargo_test` also makes `:test-run`, an action that runs
it on a worker and is cached like any other; `./scripts/verify` builds those.
`./scripts/test` runs each `:test` as an action through `buck/dev/test.bxl` and downloads only
its output, exit status and accepted paintings (`buck2 test` would download the binary and
everything it reads).

- **App tests** are the crate's tests compiled to wasm and run by `be-sdk-test-runner`; they
  read and write only their crate's `snapshots/`, so one crate's paintings leave others cached.
  **Native tests that paint** are `cargo_test(paints = True, run_env = ...)` and work the same way.
  `run_apart = True` (crates/be-host-tests) makes one action per test file, so a failing test
  loses only its own paintings (guides/testing_a_gui.md).
- Tests that draw (beui-renderer-wgpu, be-host-wayland, beui-platform-drm, ...) use lavapipe from
  `buck/sysroot:amd64-test` (`amd64-xr` adds Monado). The workers replace `LD_LIBRARY_PATH`, so a
  test whose `env` names it is wrapped by `library_path_test`.
- **Browser tests** (`web_drive_test` and `page_test` in buck/program/drive.bzl) run on this
  machine, since workers have no browser, and need Node, `playwright` and Chromium
  (guides/testing_a_gui.md, 7).
- **Compile-fail tests** live in `compile_fail/`, outside `crates/` so `build //crates/...` does not
  fail on them: `compile_fail(name, deps)` passes when every line marked
  `//~ ERROR <part of message>` has that error and no other line has one.
- Arguments after `--` go to the test executor: `--env NAME=VALUE`, `--test-arg=NAME`. Write the
  `=`: a spaced `--test-arg NAME` swallows every argument after it.
- A binary's tests run on threads, so tests touching process-wide state race unless they take
  turns.

## Shipping

- `:native` stages `be-host` and every app's manifest, data, module and `.cwasm`, and every
  library in their closures as `lib-<SHA-256>.wasm`; `apps.json` lists the manifests, which is
  how every platform finds its apps. Precompiles are one action per module in `:precompiled`,
  shared by every bundle, since an action's output path names its target and the same compile in
  two targets would be two cache entries.
- Every `program()` also has `[archive]`, its directory as one zstd file (crates/be-host-archive);
  `buck2 run` and `:dev` download only that and unpack it into `target/programs/<name>-<hash>/`.
- `:web` is be-host for `wasi` and the gpu shim for `wasm32` through wasm-bindgen, with the page
  from crates/be-host/web; `buck2 run` serves it with Caddy and be-server
  (guides/running/web.md, guides/hosting.md).
- The APK is assembled on a worker without Gradle (`be-tool-buck apk`), with apps only as arm64
  `.cwasm`s, and signed afterwards on the requesting machine (guides/running/android.md).
- CI ships nothing built for a device: the build helper builds APKs and native builds on request
  for crates/be-tool-launcher (guides/build_server.md).

## rust-analyzer

`./scripts/buck run //:rust-project` writes `rust-project.json` (git-ignored), covering native
builds and the apps as `wasm32-wasip1-threads`. Rerun it after adding a crate or dependency, and
use an editor extension's rust-analyzer: the toolchain's panics on this workspace.

## Traps

- **Features merge per plan.** A plan is `cargo test` for one platform, so a dev-dependency's
  features are on for the library too, and one plan covers every crate built for its platform:
  the `wasi` plan is be-host's, beui-web-demo's and be-tool-paint-review-web's at once. What must
  differ between builds of one triple needs its own platform and plan, told apart by a
  constraint. Proc macros and build scripts are one `linux-x86_64` build with the union of what
  every plan asks.
- **Pipelining.** Dependents compile against a `-Zno-codegen` metadata compile, which must agree
  with the real one on the crate's hash. A crate that takes in the absolute path of its action's
  directory (`env!("CARGO_MANIFEST_DIR")`, an `include_bytes!` of an absolute path) breaks that,
  and a dependent's link fails with E0460, "found possibly newer version of crate".
  `fixups.bzl` overlays the third-party crates that did.
- **Apps compile at opt-level 0** in dev builds and workspace crates under them at 1
  (`OPT_LEVELS` in buck/platforms/profile.bzl, every opt level the build sets), so a change
  under beui recompiles every app quickly (decisions/dev_builds_compile_apps_unoptimized.md).
- **A crate is built only where cargo planned it.** Building one for a platform whose plan does
  not include it, such as an app crate for anything but `wasi_guest`, fails when the build is
  configured, with `root//buck/constraints:planned_by_cargo unsatisfied`; add the crate to that
  plan in `buck/cargo/buckify.sh` and run `//:buckify`.
- **Lints** are rustc's, and a warning fails the compile, but verify compiles only
  `linux_x86_64` and `wasi_guest`, and runs no clippy
  (decisions/verify_compiles_one_configuration_per_target.md). Code behind `cfg(windows)`,
  `cfg(target_os = "macos")` or the HTML core is first compiled by the build helper's device builds or
  `./scripts/deploy`, unless checked by hand (guides/build_server.md). Clippy runs
  with `./scripts/buck bxl //buck/dev/workspace.bxl:subtarget -- --subtarget clippy.txt`
  (`clippy.toml` must exist).
- **Autofixes** are actions, one per crate (be-tool-fix-rust-source, then rustfmt); a fix that
  enables another shows up on the next run.
- **`UNAUTHENTICATED`** from the server is a wrong or missing key, which buck2 reports as failing
  to reach it.
- **Downloads**: every `http_archive` needs `size_bytes` as well as `sha256`, or buck2 sends a
  HEAD request per download on every new daemon. A download's output path is its
  configuration's, so one that several configurations reach (the host's, its release, the HTML
  core's, wasm's, each exec configuration) is downloaded and unpacked once for each: wrap it in
  `configured_once` (buck/tools/defs.bzl), as the sysroots, the wasi sysroot and the Rust
  components are, and as a crate's download is. buck2 keeps downloads across daemons but no
  other action without a cache, so fetching inside an action (curl, `git fetch`, cargo, zig)
  happens again on every new daemon: pin a file as a download instead where its hash is stable.
  What still fetches in an action: pdfium's sources (`git fetch`, about 125 MB), Ghostty and the
  Zig packages it builds with (about 65 MB) and wasm-bindgen's `cargo install` (about 35 MB).
- **crates.bzl's `downloads`** pins every crate Cargo.lock names, for every platform, built or
  not: `//:buckify` hands them to cargo as a vendored directory, so it needs no network while
  Cargo.lock is up to date. A lockfile that needs a crate they lack is resolved against
  crates.io, which downloads every crate again (about 240 MB).
