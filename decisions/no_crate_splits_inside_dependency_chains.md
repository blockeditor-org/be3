# No crate splits inside a dependency chain

A crate is split to make edits rebuild less only when some crate that is rebuilt often stops depending on one of the parts. Splitting a crate whose dependents all use every part, such as beui-core, beui-types, the beui component crates, be-sdk-api or be-sdk (every app reaches all of each through `beui` or be-sdk), is not done for caching.

## Why

- rustc's metadata records each source file's hash and its items' spans, so any edit to a library changes the `.rmeta` its dependents compile against, and buck2 rebuilds every crate above it. Parts that all sit beneath the same dependents rebuild them just the same.
- Measured in November 2026 over the last 300 commits (plans/split_crates_for_caching.md): even if nothing at all depended on beui-core, an average edit would recompile 7% fewer lines; for any crate outside beui, 3% or less.
- A chain of smaller crates compiles one after another, and each crate is one more set of actions, facts and BUCK rules.

## Rejected

- Moving be-sdk-type-guest's editor protocol into its own crate so be-sdk stops depending on the replica: apps depend on their type crates, which depend on be-sdk-type-guest, so it saved nothing.
- Splitting one test binary into several by area when the parts depend on the same crates: almost no commit edits only tests, and each binary is linked again on every library edit (be-host-stack's test harness link took 24 s, longer than compiling the library).
