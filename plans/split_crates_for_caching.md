# Split crates for caching

Depends on: none. Step 6 overlaps plans/shared_crates_per_platform.md, step 2 (be-sdk on
be-block-content only); whichever lands first, the other rebases.

Make an edit recompile and rerun less, by cutting edges in the crate graph that an edit's
rebuild follows without needing to. Measured in November 2026, before any of it.

## What an edit costs now

- **No early cutoff.** rustc's metadata records each source file's hash and its items' spans,
  so any edit to a crate's library changes the `.rmeta` its dependents compile against, and
  every crate above it in the graph recompiles, links and reruns its tests. Test files are not
  library sources (`cargo_library_sources` in buck/cargo/defs.bzl), so an edit to only a test
  rebuilds only that crate's test binary.
- **The unit is an edit to one crate**, since agents run verify after each step. The last 300
  commits (Oct 31 to Nov 15) split into 2,863 such per-crate edits. Over both platforms verify
  compiles (`linux_x86_64` and the apps' `wasi_guest`, 503k and 338k first-party library
  lines), one edit recompiles on average 78k library lines (sum over the crates it rebuilds)
  and rebuilds 22 test targets. Of the recompiled lines, host crates on Linux are 25%, apps on
  wasm 20%, beui on Linux 15% and on wasm 12%.
- **The graph is a chain.** Every app reaches beui-core, beui-types, the component crates,
  be-sdk-api, be-sdk and be-sdk-type-guest (through its own type crate), and the facade
  (`beui`) and the SDK re-export all of them. The most an edit to one of them could save,
  even if nothing depended on it at all, is 7% of the average edit (beui-core) and 3% or less
  for any crate outside beui. So no split inside that chain is worth its disruption
  (decisions/no_crate_splits_inside_dependency_chains.md); what pays is cutting an edge into
  a crate that is rebuilt often and does not use what it depends on, and compiling each app
  once rather than twice.
- **The most rebuilt large crates** (share of edits that rebuild each, times its library
  lines): be-host 34% of 24k, be-host-runtime 29% of 17k, be-host-stack 15% of 16k,
  be-host-wayland 14% of 14k, be-host-desktop 29% of 4.6k. be-host's own test binary reruns on
  44% of edits, be-host-runtime's on 34%, be-host-stack's on 29%, beui-host's (1,102 tests,
  62k lines) on 15%.
- **Linking dominates big native test binaries.** From buck2's cached wall times: linking
  be-host-stack's `:test-harness` took 24 s and be-server's 16 s, more than compiling either
  library (be-server's rlib 9 s). Splitting one test binary into several multiplies that link
  for every library edit, so a test binary is split only where its parts depend on different
  crates.

### How it was measured

`git log -300 --no-merges --name-only`, each commit cut into one edit per crate touched (test
files apart from library files); each crate's dependencies per platform from its `PACKAGE`
(`deps`, `test_deps` for `linux-x86_64` and `wasi-guest`); an edit rebuilds the crates above
it, and a test target when its crate or a test dependency is rebuilt; library lines are
`src/**/*.rs` outside `tests`. A step's saving is that simulation rerun with its edges
changed. Compile and link times are the `execution_time_us` of cached actions in the event
log of `./scripts/buck build --materializations=none` of those targets (guides/performance.md,
Measuring). Lines are a proxy for CPU; each step measures its own result as below.

## Steps

Ranked by saving per disruption. Each is one pull request by one agent, independent of the
others unless it says so, and each ends with `./scripts/verify` and with the guides that name
the moved code updated. Before and after a step, compare one cold build of what it touches:
the CPU of the actions it removes from a typical edit (guides/performance.md, Measuring), and
say the result in the pull request.

### 1. The embedded server leaves be-host-stack

be-host-stack depends on be-server only for `embedded_server.rs` (60 lines), which be-host's
`main.rs` starts and be-host-stack's own tests use. Move it into a crate of its own,
`be-host-server` (be-host family, buck/cargo/layers.bzl), that be-host depends on and
be-host-stack takes as a dev-dependency; be-host-stack keeps be-server as a dev-dependency.

- Saves: on each edit to be-server (39 of 300 commits), recompiling be-host-stack (16k
  lines), be-host-runtime (17k) and be-host-desktop (4.6k). 0.7% of all recompiled lines.
- Disruption: one file and three users. No API changes.

### 2. be-host's login session becomes a crate

`crates/be-host/src/login_session/` (2,716 lines) uses only be-host-base, be-host-services,
beui-platform-drm and three of be-host's small modules (`arguments`, `build`, `program_id`,
which move with it or are passed in). Move it to `be-host-login-session`.

- Saves: 11% of be-host's compile each time be-host is rebuilt (34% of edits, mostly for an
  edit to beui, the runtime or the stack, none of which the login session uses). About 1.2%
  of all recompiled lines.
- Disruption: small; be-host was edited in 71 of 300 commits, but the login session only in 5,
  so pick a quiet hour for be-host's `main.rs`.

### 3. be-host-runtime depends on the engine crates, not the facade

be-host-runtime depends on `beui` and `beui-host`, but uses no component, dock or keyboard:
only types (`Forwarded`, `Pos`, `KeyPress`, ...), `beui::native` (re-exports of beui-core)
and from beui-host `GpuSetup`, `SystemFonts`, `FontSources` and `drive`, which live in
beui-renderer-wgpu, beui-font-freetype and beui-layer-drive. Depend on those and on
beui-core, beui-types and beui-runner directly.

- Saves: be-host-runtime's library (17k lines) and its test binary (26k lines, its tests
  rerun) are no longer rebuilt by an edit to beui-components-unstyled or -styled, beui-dock,
  beui-dock-host, beui-keyboard, beui-inspector, beui-layer-space, beui-layer-xr or a platform
  (80 of 300 commits touched one). 1.4% of recompiled library lines and 1.8k test
  lines an edit.
- Needs the user's yes first: decisions/beui_facade_excludes_the_engine.md names "the app
  runtime" among what uses beui-host, and the host crates would then spell beui's types
  through `beui_core`/`beui_types` as beui's own engine crates do, rather than through `beui`.
  No new API, but the decision changes, and a check (buck/cargo/layers.bzl or
  be-tool-fix-rust-source) should keep the rule one rule: which crates may import beui's
  engine crates directly.

### 4. Pilot: an app compiled once, not twice

An app crate is compiled for `wasi_guest` twice on every edit beneath it: once as its library
for `:module`, once again with its tests for the test module (`app_tests` in
buck/wasm/defs.bzl). A beui or SDK edit rebuilds on average 5.5 apps, about 15k app lines on
wasm an edit (a fifth of all recompiled lines), each compiled twice. guides/performance.md
puts checklist's test module at about 13 CPU s of rustc, nearly all of it the crate itself.

Try it on one app (checklist, then text): its tests become a test crate of their own that
links the app's library rlib as the module does, with what they reach (`crate::root` and a
few modules in most apps) made `pub` or reached through the app's entry. Measure the CPU of a
beui edit's rebuild of that app before and after. If it saves what it should (about half of
each app's per-edit compile), write the rest as a follow-up plan: one app per step,
be-tool-fix-rust-source's test splitting taught the new place, guides/adding_an_app.md and
guides/testing_a_gui.md updated. If tests need private items in many places, or the app's
`cfg(test)` hooks cannot move, record that in decisions/ and stop.

- Saves, if it holds for every app: on the order of 200 CPU s for each edit under beui or the
  SDK. The largest saving here by far, and the largest change; hence a pilot.
- Disruption: one app per step; touches only that crate and its BUCK file.

### 5. beui-host's tests depend on only what they use

beui-host's 1,102 tests (62k lines, 43 CPU s) are one binary that depends on the whole
engine. 64 of them use the space layer, 56 the inspector, 2 XR; none use winit, Android or
the web. Move the rest into a test crate that depends on beui, beui-runner,
beui-platform-headless, beui-font-freetype, beui-renderer-wgpu and beui-layer-drive, and keep
the space, inspector and XR tests in beui-host.

- Saves: an edit to beui-layer-space, beui-layer-xr, beui-inspector or a beui platform (29
  of 300 commits) reruns about 120 tests and recompiles about 6k test lines instead of all
  of them.
- First check: `native::build` installs the inspector and the keyboard host into every
  document the tests build; the moved tests must build their documents the same way without
  the inspector, or this step saves nothing. If they cannot, drop the step.
- Disruption: moving files; one more test binary, so one more link per beui edit (measure it
  against what the step saves).

### 6. Apps that use no built-in stop depending on be-block

plans/shared_crates_per_platform.md, step 2: be-sdk depends on be-block-content only. Besides
the size it was written for, it stops an edit to be-block (27 of 300 commits) from rebuilding
the apps that use no built-in (about two thirds of them). Done there, not here.

## Ruled out

- **Splitting crates inside the beui chain or the SDK** (beui-core, beui-types, the component
  crates, be-sdk-api, be-sdk): every app depends on all of each through the facade or the SDK,
  so the pieces would rebuild together (decisions/no_crate_splits_inside_dependency_chains.md).
  A crate's own compile gets smaller, at the cost of more crates in a chain that compiles one
  after another.
- **The editor protocol out of be-sdk-type-guest** (`editor.rs`, which be-sdk uses): apps
  reach be-sdk-type-guest through their own type crates anyway, so the simulation saved
  nothing (-0.3%).
- **be-host-stack's terminal-host tests in a crate of their own**, to drop be-tool-cli from
  its test dependencies: it saves relinking be-host-stack's test binary on be-tool-cli edits
  (23 of 300) but adds a link as large to every be-host-stack edit (73 of 300).
- **Splitting a big test binary by area** (be-server's, be-block-client's, be-host-stack's),
  when its parts depend on the same crates: almost no commit edits only tests (at most 4 of 300 for
  any crate), so the split saves nothing on a library edit and multiplies the link.
- **An interface crate for be-host-stack's functions** so be-host-runtime need not rebuild on
  its edits: a second way to reach the same functions, for one dependent.
