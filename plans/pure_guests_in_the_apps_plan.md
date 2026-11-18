# Pure guests in the apps' plan

Depends on: none. It needed every guest built for `wasm32-wasip1`, which they are
(decisions/guests_are_single_threaded.md).
plans/host_as_platform_adapter.md's step 4g2 settled that a root is built as an app is, in
`wasi-guest` already (decisions/a_root_links_beui_as_an_app_does.md).

Build the pure guests (games, type modules, the remapper, be-wasm-root-testing's root;
guides/app_libraries.md, Pure guests) in the `wasi-guest` plan, for `wasm32-wasip1`, instead of in a `wasm32` plan of their
own, so a crate they share with the apps compiles once instead of once per plan. Do it only if
step 1 shows it saves compile time; otherwise drop this plan and record why in decisions/.

Why: a plan is one feature resolution for one triple, and the same crate in two plans is two
compiles. At 86f723ddf, about 100 crates are built in both `wasm32` and `wasi-guest` with the
same features:
- 50 first-party crates, among them every `be-type-*` crate, the block model and content crates
  (`be-block-model`, `-content`, `-graph`, `-protocol`), `be-sdk-type-guest`, `be-sdk-type-api`,
  `be-lib-ui-types` and `be-wasm-guest-abi`;
- 49 third-party crates, among them serde, postcard, ciborium, toml_edit, sha2 and hashbrown.

The type crates change with most app work, so each such change compiles them twice. Before guests
were single-threaded the apps' triple was `wasm32-wasip1-threads`, which a pure guest could not be
built for, so they could not share a plan at all.

## What changes for a pure guest

Rust's std on wasip1 imports a few `wasi_snapshot_preview1` functions (`fd_write`, `proc_exit`,
`environ_*`, `args_*`, `random_get`, `clock_time_get` where read). A pure guest's host answers
them instead of refusing the module: `random_get` from the seeded randomness type modules already
get, `fd_write` to the log, the clock and environment with "not supported", and `proc_exit`
stops the guest. Answers stay deterministic, so a type module stays one. Every other import is
still refused. This touches be-wasm-host's pure-guest validation and the sandbox's `compile`
(guides/apps/runtime.md, Backends), and their web halves.

Feature unification is the cost. In one plan, the pure guests get the apps' features. At
86f723ddf four crates differ:
- `be-block-store` (`default`, `vault`);
- `be-block-commit` (`store`);
- `be-block-session` (`default`, `store`);
- `be-wasm-gpu-abi` (`wgpu`).

Check what each feature brings into a type module (size, and any import beyond the answered
WASI set). Where one brings in something a pure guest must not have, split the crate rather than
keep a second plan.

The browser's gpu shim keeps its own `wasm32-bindgen` plan: it is wasm-bindgen code for
`wasm32-unknown-unknown`, not a guest.

## Steps

1. Measure: on the build server, build every pure guest and every app from cold, once as today
   and once with the pure guests moved into `wasi-guest` (a scratch branch is enough). Then
   compare a typical edit, a change to one `be-type-*` crate and to `be-block-model`, by compile
   actions and CPU seconds (guides/performance.md). Also compare the pure guests' module sizes,
   to price the unified features. Go on only if the edits save time.
2. Answer the WASI set for pure guests natively and on the web, with tests that a type module
   built for wasip1 loads, runs deterministically, and has any other import refused.
3. Move the pure guests into `wasi-guest` in `crates/be-tool-buck/src/plan.rs`, move
   `wasm32_module` to the wasip1 platform in `buck/wasm/defs.bzl`, delete the `wasm32` plan and
   platform, and run `./scripts/buckify`. Update guides/buck2.md, guides/app_libraries.md
   (Pure guests) and guides/apps/runtime.md.
