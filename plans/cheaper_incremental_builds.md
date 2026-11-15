# Cheaper incremental builds

Depends on: none.

Make an edit recompile, relink and rerun less: by cutting edges in the crate graph that an
edit's rebuild follows without needing to, and by linking big native test binaries against
shared libraries. Measured in November 2026, before any of it.

Landed: beui's windowed runner (winit, Android, and the space, XR and drive layers it stacks)
moved out of beui-host into beui-host-window, and the space host's tests through a `Runner`
into beui-layer-space (guides/beui/programs.md, Windows and the web). Of beui-host's 1,102
tests, 11 used the space layer (not 64, as this plan first said: that count matched beui-core's
layer space) and none XR or winit; all of them need beui-host's `build`, which installs the
inspector, so no separate test crate could have dropped those layers while beui-host's own
`window` feature pulled them in. After a no-op edit to beui-layer-space, the build ran 21
actions where it ran 68 (rustc's share 54 s of wall time where it was 162 s), and 8 test
binaries rerun where 20 did, beui-host's 1,092 tests (37 CPU s) among those no longer rerun.

## What an edit costs now

- **No early cutoff.** rustc's metadata records each source file's hash and its items' spans,
  so any edit to a crate's library changes the `.rmeta` its dependents compile against; and
  each dependent's metadata records the hashes of the crates it was compiled against, so its
  own `.rmeta` changes too, and every crate above the edit recompiles, links and reruns its
  tests. Test files are not
  library sources (`cargo_library_sources` in buck/cargo/defs.bzl), so an edit to only a test
  rebuilds only that crate's test binary.
- **The unit is an edit to one crate**, since agents run verify after each step. The last 300
  commits (Oct 31 to Nov 15) split into 2,863 such per-crate edits. Over both platforms verify
  compiles (`linux_x86_64` and the apps' `wasi_guest`, 503k and 338k first-party library
  lines), one edit recompiles on average 78k library lines (sum over the crates it rebuilds)
  and rebuilds 22 test targets. Of the recompiled lines, host crates on Linux are 25%, apps on
  wasm 20%, beui on Linux 15% and on wasm 12%. The one-crate model overstates the saving of
  a split high in the graph, since a commit that touches it usually touches crates above it
  too; each step gives its saving per commit as well (the 261 of those commits that touch
  Rust: 254k library lines a commit).
- **The graph is a chain.** Every app reaches beui-core, beui-types, the component crates,
  be-sdk-api, be-sdk and be-sdk-type-guest (through its own type crate), and the facade
  (`beui`) and the SDK re-export all of them. The most an edit to one of them could save,
  even if nothing depended on it at all, is 7% of the average edit (beui-core) and 3% or less
  for any crate outside beui; and a split's own compile saving is small too (halving
  beui-core would save about 0.5%). So no split inside that chain is worth its disruption
  (decisions/no_crate_splits_inside_dependency_chains.md); what pays is cutting an edge into
  a crate that is rebuilt often and does not use what it depends on. Compiling each app once
  rather than twice was piloted and not adopted (decisions/app_tests_compile_with_their_app.md).
- **The most rebuilt large crates** (share of edits that rebuild each, times its library
  lines): be-host 34% of 24k, be-host-runtime 29% of 17k, be-host-stack 15% of 16k,
  be-host-wayland 14% of 14k, be-host-desktop 29% of 4.6k. be-host's own test binary reruns on
  44% of edits, be-host-runtime's on 34%, be-host-stack's on 29%, beui-host's (1,102 tests,
  62k lines) on 15%.
- **Linking dominates big native test binaries.** From buck2's cached wall times: linking
  be-host-stack's `:test-harness` took 24 s and be-server's 16 s, more than compiling be-server's
  library (9 s). Splitting one test binary into several multiplies that link for every library
  edit, so a test binary is split only where its parts depend on different crates. Linking
  test binaries against shared libraries instead needs a toolchain-wide change
  (decisions/native_test_binaries_link_statically.md).

### How it was measured

`git log -300 --no-merges --name-only`, each commit cut into one edit per crate touched (test
files apart from library files); each crate's dependencies per platform from its `PACKAGE`
(`deps`, `test_deps` for `linux-x86_64` and `wasi-guest`); an edit rebuilds the crates above
it, and a test target when its crate or a test dependency is rebuilt; library lines are
`src/**/*.rs` outside `tests`. A step's saving is that simulation rerun with its edges
changed, a new crate given the whole set of dependencies its moved code uses, not just the
ones it names most. Compile and link times are the `execution_time_us` of cached actions in the event
log of `./scripts/buck build --materializations=none` of those targets (guides/performance.md,
Measuring). Lines are a proxy for CPU; each step measures its own result as below.

## Steps

Ranked by saving per disruption. Each is one pull request by one agent, independent of the others, and each ends with
`./scripts/verify` and with the guides that name the moved code updated. Before and after a
step, measure the CPU of the actions an edit it targets runs (guides/performance.md,
Measuring), and put the difference in the landing commit's message. Savings are from the
simulation, per one-crate edit and per commit.

### 1. The embedded server leaves be-host-stack

be-host-stack depends on be-server only for `embedded_server.rs` (81 lines), which be-host's
`main.rs` starts, be-host-testing uses (`be_host_stack::embedded_server`) and be-host-stack's
own tests use. Move it into a crate of its own, `be-host-server` (be-host family,
buck/cargo/layers.bzl), that be-host and be-host-testing depend on and be-host-stack takes as a
dev-dependency; be-host-stack keeps be-server as a dev-dependency.

- Saves: on each edit to be-server (39 of 300 commits), recompiling be-host-stack (16k
  lines), be-host-runtime (17k) and be-host-desktop (4.6k). 0.7% of recompiled library lines
  per edit, 0.2% per commit.
- Disruption: one file and four users. No API changes.
- Done: be-host-stack's library has no dependency on be-server (its `PACKAGE` lists it only
  under `test_deps`); be-host and be-host-testing start the server through `be-host-server`;
  the landing commit gives the CPU an edit to be-server runs, before and after.

### 2. be-host-runtime depends on the engine crates, not the facade

be-host-runtime depends on `beui` and `beui-host`, but uses no component, dock or keyboard:
only types (`Forwarded`, `Pos`, `KeyPress`, ...), `beui::native` (re-exports of beui-core),
beui-draw's `Draw`, `painted_drawing` and `texture_quad`, accesskit, and from beui-host
`GpuSetup`, `SystemFonts`, `FontSources` and `drive`, which live in beui-renderer-wgpu,
beui-font-freetype and beui-layer-drive. Depend on those and on beui-core, beui-types,
beui-draw, beui-runner and accesskit directly.

- Waiting for the user's OK: it cuts be-host-runtime's rebuilds on component and engine-layer
  edits, at the cost of the runtime spelling beui's types through the engine crates
  (`beui_types::Pos`) where every other crate outside beui writes `beui::Pos`, an allowlisted
  second spelling. The plan lands with this step marked; nobody starts it until the user
  says yes.
- Saves: be-host-runtime's library (17k lines) and its test binary (26k lines, its tests
  rerun) are no longer rebuilt by an edit to beui-components-unstyled or -styled, beui-dock,
  beui-dock-host, beui-keyboard, beui-inspector, beui-layer-space, beui-layer-xr or a platform
  (80 of 300 commits touched one). 1.7% of recompiled library lines and 2.0k test lines per
  edit; 0.6% and 2.2k per commit.
- The step amends decisions/beui_facade_excludes_the_engine.md as it lands: the app runtime is
  engine code, like beui-runner and the platforms, and depends on the engine crates beneath
  the components (beui-core, beui-types, beui-draw, beui-runner, beui-renderer-wgpu,
  beui-font-freetype, beui-layer-drive, and accesskit), spelling their items through those
  crates; it does not depend on `beui` or `beui-host`. Programs, demos and test harnesses
  keep using beui-host. One spelling per crate: a crate that depends on the engine crates
  does not also depend on the facade. The check that holds it (buck/cargo/layers.bzl or
  be-tool-fix-rust-source) names the crates outside beui allowed to depend on the engine
  crates.
- Done: be-host-runtime's `PACKAGE` has neither `beui` nor `beui-host`; the decision says the
  above and the check enforces it; the landing commit gives the CPU an edit to
  beui-components-styled runs, before and after.

## Note: apps that use no built-in

plans/shared_crates_per_platform.md, step 2, makes be-sdk depend on be-block-content only. For
caching it matters only if be-sdk-beui's dependency on be-block goes too: cutting be-sdk's edge
alone frees 1 of 28 apps from rebuilding on a be-block edit (27 of 300 commits), since
be-sdk-beui also depends on be-block; cutting both frees 20 of 28, the eight apps that use a
built-in depending on be-block themselves. The simulation gives 0.5% of recompiled library
lines per edit for be-sdk's edge alone and 2.4% for both (0.3% and 0.9% per commit).

## Ruled out

- **Big native test binaries linked against shared libraries**: buck2's prelude links Rust
  dependencies as shared objects only under a toolchain-wide setting that also needs the
  standard library as explicit sysroot dependencies on every platform
  (decisions/native_test_binaries_link_statically.md).
- **An app compiled once, its tests a crate of their own**: piloted on checklist and
  infinite-canvas; it saves a share of each app's compile but makes everything its tests reach
  public and rewrites white-box tests in 12 apps (decisions/app_tests_compile_with_their_app.md).
- **Splitting crates inside the beui chain or the SDK** (beui-core, beui-types, the component
  crates, be-sdk-api, be-sdk): every app depends on all of each through the facade or the SDK,
  so the pieces would rebuild together (decisions/no_crate_splits_inside_dependency_chains.md).
  A crate's own compile gets smaller, at the cost of more crates in a chain that compiles one
  after another.
- **The editor protocol out of be-sdk-type-guest** (`editor.rs`, which be-sdk uses): apps
  reach be-sdk-type-guest through their own type crates anyway, so the simulation saved
  nothing (-0.3%).
- **be-host's login session as a crate of its own** (2,554 library lines): its dependencies
  reach be-host-desktop, beui and beui-platform-drm, so it would rebuild on nearly every edit
  that rebuilds be-host. 0.1% of recompiled lines per edit.
- **be-host-stack's terminal-host tests in a crate of their own**, to drop be-tool-cli from
  its test dependencies: it saves relinking be-host-stack's test binary on be-tool-cli edits
  (23 of 300) but adds a link as large to every be-host-stack edit (73 of 300).
- **Splitting a big test binary by area** (be-server's, be-block-client's, be-host-stack's),
  when its parts depend on the same crates: almost no commit edits only tests (at most 4 of 300 for
  any crate), so the split saves nothing on a library edit and multiplies the link.
- **An interface crate for be-host-stack's functions** so be-host-runtime need not rebuild on
  its edits: a second way to reach the same functions, for one dependent.
