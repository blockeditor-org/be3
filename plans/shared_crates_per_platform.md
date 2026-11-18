# Shared crates per platform

Depends on: none.

`./scripts/verify` builds a shared crate once for every platform something
under a test needs it on, so a crate that is only reachable through a dep
that is wider than it needs costs a full compile per platform for nothing.
This plan stops building be-block and similar shared crates where they are
not needed.

## The platforms verify builds

verify (`buck/dev/verify.bxl`) builds every test on its own default platform,
and what those tests depend on:

- `linux_x86_64` (and `linux_x86_64_html` for the HTML core's tests):
  everything.
- `wasi_guest` (wasm32-wasip1, `buck/constraints:guest`): the apps, which the
  host tests and the apps' own tests load.
- `wasm32` (wasm32-unknown-unknown, no WASI): the type modules (`:module`),
  which the host tests and be-sdk-test load.
- Execution platform: proc macros and build tools.

It does not build the web (`wasi` / `wasi_html`: `//crates/be-host:web` and
its program modules): no test depends on them, so a web build break
passes verify, and stays out of it (Ruled out, below).

## Which shared crates build where, and why

After the be-block-content split, the type modules (`wasm32`) build 72 Rust
libraries, down from 93 (the type crates themselves among them). They no
longer build be-block, be-lib-search, be-lib-graphemes,
be-lib-ui-preferences, caseless, unicode-normalization, tinyvec or the icu
crates. What is left is used:

| crate | type modules use it for |
|---|---|
| be-block-content | the traits, metadata, `Root`, blobs |
| be-block-model, be-lib-sequence | `Document<R>`, text and lists |
| be-block-commit (no `store`), `similar` | `MergeResult`; `similar` is the line diff under be-block-model's sequence merge (`merge_lines`) |
| be-block-store (no `vault`), sha2, ciborium | `Part`/`PartRef`/`Hash`/`Manifest`, CBOR of the model |
| be-block-session (no `store`) | the order (`Sequencer`) and `SessionMessage` the replica runs |
| be-block-protocol, be-block-graph | `SessionState`, `ClientId`, `RelayKind`; graph only for `Access`/`BlockParent` in the protocol, and `WORKSPACE_TYPE` |
| be-sdk-type-api, be-wasm-ops, be-wasm-guest-abi | the protocol and the guest ABI |
| be-lib-ui-keys, logicgame, rand | be-type-terminal and be-type-logic-grid's own needs |

The apps (`wasi_guest`) build be-block, which is now only the built-ins every
editor's frame reads: be-sdk re-exports it (`be_sdk::be_block`) and
be-sdk-beui's frame resolves settings, profiles, display settings, organization
and workspace blocks and tags through it for every editor, so it stays under
the SDK (it is about 1,700 of be-block's 2,500 lines that the frame uses, and
settings needs device_class). The built-ins only some editors read are crates of
their own, so an edit to one rebuilds no app but its own: be-block-input (input
settings and the remapper: the settings app and the hosts), be-block-repository
(repositories and checkouts: the version control app and the hosts) and
be-block-search-index (hosts, the server and the test library), and
`quotable_name` is be-lib-quoted-name's (be-host and be-sdk-shell). be-lib-search,
caseless and unicode-normalization are in no app's build; be-lib-graphemes and
the icu crates are, through beui.

## Steps

1. Split be-block-commit's host-only parts from what the replica uses: done.
   `retention` sits behind the `store` feature with `CommitStore`, which no
   type module enables, and the time spans (`MINUTE`, `HOUR`, `DAY`, `WEEK`)
   moved to the crate root, which be-block and be-block-search-index read
   without it. Only a compile-time win, as expected: the dev `:module`s did
   not change size beyond noise (chat 4,166,413 to 4,166,001 bytes, checklist
   3,433,545 to 3,434,000, text 3,547,056 to 3,547,518), since the linker
   already dropped them.
2. `similar` in type modules: replacing it means writing the line diff that
   `merge_lines` needs (Myers over slices) in be-block-model or
   be-lib-sequence. That is a design change, not a split: worth it only if
   it measurably shrinks the modules (`similar` was ~190 KB of a dev
   checklist; release drops most of it).

## Ruled out

- **Building the web in verify** (`//crates/be-host:web`, or one target that depends on it): it
  would catch a web build break, at the cost of every web crate's wasm build on each verify. The
  user decided against it (November 2026); `./scripts/buck build //crates/be-host:web` checks a
  web-specific change by hand.

## Size levers for type modules

Measured on the release `:module` of be-type-checklist (2.29 MB: code 2.07 MB,
name section 0.17 MB):

- opt-level: done. First-party crates are built at `s` in a release build on
  wasm32-unknown-unknown (`member` in `OPT_LEVELS`, buck/platforms/profile.bzl,
  which says why): 1.67 MB, every type module a quarter to a third smaller.
  Apps and their libraries keep 3, where `s` and `z` slowed their frames.
- The name section stays (0.07–0.20 MB a module at 3, more at `s`): be-wasm-host
  names a guest's backtrace and `BE3_PERF_MAP`'s profile from it
  (decisions/shipped_bundle_size_settings.md).
