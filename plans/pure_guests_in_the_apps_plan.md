# Pure guests in the apps' plan

Depends on: none. It needed every guest built for `wasm32-wasip1`, which they are
(decisions/guests_are_single_threaded.md).
plans/host_as_platform_adapter.md's step 4g2 settled that a root is built as an app is, in
`wasi-guest` already (decisions/a_root_links_beui_as_an_app_does.md).

Build the pure guests (games, type modules, the remapper, be-wasm-root-testing's root;
guides/app_libraries.md, Pure guests) in the `wasi-guest` plan, for `wasm32-wasip1`, instead of in a `wasm32` plan of their
own, so every guest is built for one triple, WASI, and a crate they share with the apps
compiles once instead of once per plan.

Why: every guest on one triple, and that triple WASI, since the C and C++ libraries guests link
need a libc and some Rust crates take `wasm32-unknown-unknown` to mean a web page
(decisions/a_root_links_beui_as_an_app_does.md). And a plan is one feature resolution for one
triple, and the same crate in two plans is two compiles. At 86f723ddf, about 100 crates are built in both `wasm32` and `wasi-guest` with the
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

1. Answer the WASI set for pure guests natively and on the web, with tests that a type module
   built for wasip1 loads, runs deterministically, and has any other import refused.
   Built: `crates/be-wasm-pure-wasi` answers the set (randomness a ChaCha20 stream from a seed,
   `Host::with_seed`, zero unless given), used by be-wasm-host, tabletop-host's wasmi and
   be-wasm-root-host's page guest; be-wasm-guest-host's `guest.js` answers the same stream in
   JavaScript, and the page's sandbox `compile` lets the set through. Tested with hand-written
   guests (be-wasm-host's `a_pure_guest_is_answered_its_few_wasi_calls_from_its_seed`,
   `..._may_not_import_more_of_wasi_than_it_is_answered`, tabletop-host's
   `a_game_may_import_a_pure_guests_wasi_and_no_more`); the type modules themselves are built
   for wasip1 by step 2, whose tests run them so.
2. Move the pure guests into `wasi-guest` in `crates/be-tool-buck/src/plan.rs`, move
   `wasm32_module` to the wasip1 platform in `buck/wasm/defs.bzl`, delete the `wasm32` plan and
   platform, and run `./scripts/buckify`. Update guides/buck2.md, guides/app_libraries.md
   (Pure guests) and guides/apps/runtime.md.
   Built: every pure guest is in `wasi-guest` and `pure_guest_crate` (was `wasm32_crate`) links
   its `:module` there; the `wasm32` plan and platform are gone. 57 first-party and 49
   third-party crates the two plans shared now compile once. On wasip1 getrandom and std's hash
   keys read `random_get`, so be-block-type-host seeds each module through its host
   (`Host::with_seed`, `WebGuest::start_pure`) and `replicas.start` takes no entropy;
   be-sdk-type-guest's generator and the getrandom fixup are gone. Of the four crates whose
   features differed, `vault` and `store` add no import and no size (the dev type modules came out
   about 1% smaller), but be-wasm-gpu-abi's `wgpu` linked wgpu, and with it wasm-bindgen, into
   be-wasm-root-testing, so the conversions moved to `crates/be-wasm-gpu-convert` (traits
   `Convert`/`TryConvert`, since neither side's types are its own). Games take simd128 with the
   guests, so tabletop-host's wasmi has its `simd` feature, with relaxed SIMD off. Release builds
   compiled os:none crates (the pure guests) at opt-level `s` and WASI ones at 3; in one plan
   the pure guests take 3 (decisions/shipped_bundle_size_settings.md, Since), for now: `s` for
   the guests' own crates only, or for the apps' too, awaits the owner's choice.
