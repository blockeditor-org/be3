# Cheaper incremental builds

Depends on: none.

Make an edit recompile, relink and rerun less: by cutting edges in the crate graph that an
edit's rebuild follows without needing to, and by linking big native test binaries against
shared libraries. Measured in November 2026, before any of it.

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
  edit, so a test binary is split only where its parts depend on different crates; making the
  link itself cheaper is step 4.

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

Steps 1 to 3 are ranked by saving per disruption; step 4 is a build-rule pilot of its own.
Each is one pull request by one agent, independent of the others, and each ends with
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

### 3. beui-host's tests depend on only what they use

beui-host's 1,102 tests (62k lines, 43 CPU s) are one binary that depends on the whole
engine. 64 of them use the space layer, 56 the inspector, 2 XR; none use winit, Android or
the web. Move the rest into a new crate, `beui-headless-tests`, that depends on beui,
beui-runner, beui-platform-headless, beui-font-freetype, beui-renderer-wgpu and
beui-layer-drive, and keep the space, inspector and XR tests in beui-host. One rule, written
into guides/beui/core.md (Testing beui): an in-process beui test goes in
`beui-headless-tests` unless it uses the space layer, the inspector or XR, which put it in
beui-host; crates/beui-tests stays what it is, screens that both cores are driven through.

- Saves: an edit to beui-layer-space, beui-layer-xr, beui-inspector or a beui platform (29
  of 300 commits) reruns about 120 tests and recompiles about 6k test lines instead of all
  of them.
- First check: `native::build` installs the inspector and the keyboard host into every
  document the tests build; the moved tests must build their documents the same way without
  the inspector, or this step saves nothing. If they cannot, drop the step.
- Disruption: moving files; one more test binary, so one more link per beui edit (measure it
  against what the step saves).
- Done: beui-host's own tests are only those that use the space layer, the inspector or XR;
  the rule is in guides/beui/core.md; the landing commit gives the CPU an edit to
  beui-layer-space runs, before and after.

### 4. Pilot: big native test binaries linked against shared libraries

Every edit beneath a native test binary relinks it statically with all of its dependencies
(wasmtime, cranelift, tokio, SQLite...). From cached wall times, be-host-stack's
`:test-harness` link took 24 s and be-server's 16 s, longer than compiling be-server's library
(9 s); be-host-stack's harness is relinked on 29% of edits. The test still reruns after such an
edit (its inputs changed), and with no early cutoff every crate above the edit still
recompiles and relinks its own shared object. What a dynamic link saves is relinking what lies
below the edit and has not changed: wasmtime, cranelift, tokio, SQLite and the first-party
crates beneath the edited one, which a static link copies in again every time.

Pilot on those two harnesses: link the test binary dynamically, with first-party and
third-party crates as shared objects (the prelude's shared link style for Rust, or
`-C prefer-dynamic` with dylib crate types; whichever buck2's prelude supports without
patching), and only for tests, never for what ships. Things to check:

- The workers replace `LD_LIBRARY_PATH`; the launcher's `--library-path` (guides/buck2.md,
  Tests) must carry the shared objects' directory, and a test's runtime inputs must include
  them so remote execution stages them.
- Pipelining and the crate hash (guides/buck2.md, Traps): a dylib build of a crate must not
  break the E0460 agreement between its metadata and its real compile.
- Each crate built as a shared object is one more action per crate, compiled once and shared
  by every harness that links it; a crate built both ways is compiled twice. Count that
  against the link saved.
- Measure, for an edit to be-server and to be-host-stack: the CPU of the harness link, of the
  shared-object links of every crate above the edit, and of the extra shared-object compiles,
  before and after (guides/performance.md, Measuring), and the test's own run time, which
  loading many shared objects can lengthen.

If it saves more than it costs, follow up with one step per further harness, starting with the
largest links (from the event log of a full build), and write it into guides/buck2.md and
guides/performance.md. If not, record the measurement in decisions/ and stop.

- Disruption: build rules only (buck/cargo/defs.bzl, buck/dev/test.bzl); no crate changes.
- Done: be-host-stack's and be-server's harnesses link dynamically, or a decision records why
  not; guides/buck2.md (Tests) says those test binaries are linked differently from what
  ships, which is linked statically, so a fault in static linking shows only in a build of the
  shipped binary; the landing commit gives the CPU of an edit to be-server, before and after.

## Note: apps that use no built-in

plans/shared_crates_per_platform.md, step 2, makes be-sdk depend on be-block-content only. For
caching it matters only if be-sdk-beui's dependency on be-block goes too: cutting be-sdk's edge
alone frees 1 of 28 apps from rebuilding on a be-block edit (27 of 300 commits), since
be-sdk-beui also depends on be-block; cutting both frees 20 of 28, the eight apps that use a
built-in depending on be-block themselves. The simulation gives 0.5% of recompiled library
lines per edit for be-sdk's edge alone and 2.4% for both (0.3% and 0.9% per commit).

## Ruled out

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
