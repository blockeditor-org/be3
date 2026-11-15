# No crate splits inside a dependency chain

A crate is split to make edits rebuild less only when some crate that is rebuilt often stops depending on one of the parts. Splitting a crate whose dependents all use every part, such as beui-core, beui-types, the beui component crates, be-sdk-api or be-sdk (every app reaches all of each through `beui` or be-sdk), is not done for caching.

## Why

- rustc's metadata records each source file's hash and its items' spans, so any edit to a library changes the `.rmeta` its dependents compile against; each dependent's metadata records the hashes of the crates it was compiled against, so its own `.rmeta` changes in turn, and buck2 rebuilds the whole chain above the edit. Parts that all sit beneath the same dependents rebuild them just the same.
- Measured in November 2026 over the last 300 commits (plans/cheaper_incremental_builds.md): even if nothing at all depended on beui-core, an average edit would recompile 7% fewer lines; for any crate outside beui, 3% or less. The split crate's own compile saves little too: halving beui-core would save about 0.5%.
- A chain of smaller crates compiles one after another, and each crate is one more set of actions, facts and BUCK rules.

## Splitting many crates for caching

A proposal to split many crates so edits rebuild less was measured again on November 15, 2026,
over the last 300 commits (1,160 edits to a crate's library, on linux and the apps' wasm). An
edit recompiles on average 143k library lines and reruns 35 test targets; an edit to beui-core,
beui-types, beui-tree or beui-reactive recompiles 230k to 310k lines on each platform and
reruns 50 to 61 test targets, an edit to an app only itself. Removing every single edge in the
graph in turn, the best any could save was 4% (beui-components-unstyled on beui-view, which it
uses throughout); of the edges a crate barely used, three were cut
(plans/cheaper_incremental_builds.md, steps 1, 3 and 4), for 2.4% together. Because the
metadata changes on every edit, splitting a crate lowers what an edit rebuilds only where a
dependent stops depending on one of the parts, and in this graph few do: the large savings would
take crates that are rebuilt less, not more of them.

## Rejected

- Moving be-sdk-type-guest's editor protocol into its own crate so be-sdk stops depending on the replica: apps depend on their type crates, which depend on be-sdk-type-guest, so it saved nothing.
- Splitting one test binary into several by area when the parts depend on the same crates: almost no commit edits only tests, and each binary is linked again on every library edit (be-host-stack's test harness link took 24 s, longer than compiling the library).
