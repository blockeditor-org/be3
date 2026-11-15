# Cheaper incremental builds

Depends on: none.

Make an edit recompile, relink and rerun less, by cutting edges in the crate graph that an
edit's rebuild follows without needing to. Measured in November 2026, before any of it.

The windowed runner's move to beui-host-window, once a step here, is in guides/beui/programs.md.

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

### 1. The embedded server leaves be-host-stack (done)

be-host-stack's library used be-server only to start the server be-host embeds; that is
crates/be-host-server, which needs be-server and tokio alone, and be-host and be-host-testing
depend on it. be-host-stack keeps be-server and be-host-server as test dependencies.

- Saved: 0.78% of recompiled library lines per edit, 0.80% per commit, re-measured on Nov 15
  over the 300 commits before it. The first measurement gave 0.7% and 0.2%: per commit, a commit
  that edits be-server and also be-host-stack, be-host-runtime or be-host-desktop rebuilds them
  anyway, and every commit that edited be-server's library then did; of the 27 in the 300 since,
  18 edit none of the three. Measured on the build server, an edit to be-server no longer runs the check and
  diagnostics of be-host-stack (7.6 s), be-host-runtime (5.3 s) and be-host-desktop (5.2 s).

### 2. be-host-runtime depends on beui-documents as `beui`, not on the facade (done)

The user chose one spelling for every item. be-host-runtime depends on `beui-documents`, the
facade's surface below the components, under the name `beui`, and on beui-renderer-wgpu,
beui-font-freetype and beui-layer-drive; the facade re-exports `beui-documents` at the same paths
(decisions/beui_facade_excludes_the_engine.md, guides/beui.md).

- Saved: 1.92% of recompiled library lines per edit, 0.75% per commit (re-measured on Nov 15).

### 3. beui-dock-state depends on be-lib-geometry, not beui-types (done)

beui-dock-state used only `Direction`, `Rect` and `Pos2` of beui-types; `Direction` moved into
be-lib-geometry beside the other two, re-exported by beui-types.

- Saved: 0.93% of recompiled library lines per edit, 1.26% per commit, 0.17 test targets per
  edit. An edit to beui-types, beui-tree or beui-reactive no longer runs beui-dock-state's
  compile (2.0 s on linux, 1.0 s on wasm) or its tests.

### 4. Desktop programs and icons leave be-host-wayland (done)

be-host-wayland's `programs` module (desktop entries, MIME globs, icon themes) is
crates/be-host-programs, which the compositor, be-host-desktop and be-host-services depend on;
be-host-services used nothing else of the compositor.

- Saved: 0.72% of recompiled library lines per edit, 0.31% per commit. An edit to
  be-host-wayland no longer runs be-host-services' compile and test link (6.8 s) or its tests.

## Ruled out

- **be-sdk and be-sdk-beui off be-block**: be-sdk-beui's frame reads most of be-block (settings,
  profiles, display settings, organization and workspace blocks, tags) for every editor, so the
  edge stays; the built-ins only some editors read left be-block instead
  (plans/shared_crates_per_platform.md).
- **Big native test binaries linked against shared libraries**: buck2's prelude links Rust
  dependencies as shared objects only under a toolchain-wide setting that also needs the
  standard library as explicit sysroot dependencies on every platform
  (decisions/native_test_binaries_link_statically.md).
- **An app compiled once, its tests a crate of their own**: piloted on checklist and
  infinite-canvas; it saves a share of each app's compile but makes everything its tests reach
  public and rewrites white-box tests in 12 apps (decisions/app_tests_compile_with_their_app.md).
- **The beui facade without the dock** (apps reaching beui-dock-host through be-sdk-beui
  rather than `beui`): be-sdk-beui puts a dock under every editor, so 0.34% per edit.
- **unstyled off beui-view, beui-view off beui-html**: each uses the other throughout
  (primitives, overlays, drag; the native core's escape to HTML nodes).
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
