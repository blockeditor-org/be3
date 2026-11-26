# No crate splits inside a dependency chain

A crate is split to make edits rebuild less only when some crate that is rebuilt often stops depending on one of the parts. Splitting a crate whose dependents all use every part, such as beui-core, beui-types, the beui component crates, be-sdk-api or be-sdk (every app reaches all of each through `beui` or be-sdk), is not done for caching.

## Why

- rustc's metadata records each source file's hash and its items' spans, so any edit to a library changes the `.rmeta` its dependents compile against; each dependent's metadata records the hashes of the crates it was compiled against, so its own `.rmeta` changes in turn, and buck2 rebuilds the whole chain above the edit. Parts that all sit beneath the same dependents rebuild them just the same.
- Measured in November 2026 over the last 300 commits (How it was measured, below): even if nothing at all depended on beui-core, an average edit would recompile 7% fewer lines; for any crate outside beui, 3% or less. The split crate's own compile saves little too: halving beui-core would save about 0.5%.
- A chain of smaller crates compiles one after another, and each crate is one more set of actions, facts and BUCK rules.

## Splitting many crates for caching

A proposal to split many crates so edits rebuild less was measured again on November 15, 2026,
over the last 300 commits (1,160 edits to a crate's library, on linux and the apps' wasm). An
edit recompiles on average 143k library lines and reruns 35 test targets; an edit to beui-core,
beui-types, beui-tree or beui-reactive recompiles 230k to 310k lines on each platform and
reruns 50 to 61 test targets, an edit to an app only itself. Removing every single edge in the
graph in turn, the best any could save was 4% (beui-components-unstyled on beui-view, which it
uses throughout); of the edges a crate barely used, three were cut (the embedded server out of
be-host-stack into be-host-server, beui-dock-state onto be-lib-geometry, desktop programs
out of be-host-wayland into be-host-programs), for 2.4% together, and be-host-runtime came
to depend on beui-documents rather than the facade. Because the
metadata changes on every edit, splitting a crate lowers what an edit rebuilds only where a
dependent stops depending on one of the parts, and in this graph few do: the large savings would
take crates that are rebuilt less, not more of them.

## Rejected

- Moving be-sdk-type-guest's editor protocol into its own crate so be-sdk stops depending on the replica: apps depend on their type crates, which depend on be-sdk-type-guest, so it saved nothing.
- Splitting one test binary into several by area when the parts depend on the same crates: almost no commit edits only tests, and each binary is linked again on every library edit (be-host-stack's test harness link took 24 s, longer than compiling the library).
- The beui facade without the dock (apps reaching beui-dock-host through be-sdk-beui rather
  than `beui`): be-sdk-beui puts a dock under every editor, so it saved 0.34% per edit.
- be-sdk and be-sdk-beui off be-block: be-sdk-beui's frame reads most of be-block (settings,
  profiles, display settings, organization and workspace blocks, tags) for every editor; only
  the built-ins some editors read left be-block.
- beui-components-unstyled off beui-view, or beui-view off beui-html: each uses the other
  throughout (primitives, overlays, drag; the native core's escape to HTML nodes).
- be-host's login session as a crate of its own (2,554 library lines): its dependencies reach
  be-host-desktop, beui and beui-platform-drm, so it would rebuild on nearly every edit that
  rebuilds be-host. 0.1% of recompiled lines per edit.
- be-host-stack's terminal-host tests in a crate of their own, to drop be-tool-cli from its test
  dependencies: it saves relinking be-host-stack's test binary on be-tool-cli edits (23 of 300
  commits) but adds a link as large to every be-host-stack edit (73 of 300).
- An interface crate for be-host-stack's functions so be-host-runtime need not rebuild on its
  edits: a second way to reach the same functions, for one dependent.

## How it was measured

`git log -300 --no-merges --name-only`, each commit cut into one edit per crate touched (test
files apart from library files: they are not library sources, `cargo_library_sources` in
buck/cargo/defs.bzl, so an edit to only a test rebuilds only that crate's test binary); each
crate's dependencies per platform from its `PACKAGE` (`deps`, `test_deps` for `linux-x86_64`
and `wasi-guest`); an edit rebuilds the crates above it, and a test target when its crate or a
test dependency is rebuilt; library lines are `src/**/*.rs` outside `tests`. A proposed change
is that simulation rerun with its edges changed, a new crate given the whole set of
dependencies its moved code uses. The one-crate model overstates a split high in the graph,
since a commit that touches it usually touches crates above it too, so give the saving per
commit as well. Lines are a proxy for CPU: measure an adopted change's CPU before and after
(guides/performance.md, Measuring).
