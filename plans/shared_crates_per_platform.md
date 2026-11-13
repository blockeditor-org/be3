# Shared crates per platform

Depends on: none. (be-block-content, which split the type modules' content
core out of be-block, has landed.)

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
its program modules): no test depends on them, which is how a web build
break on main went unnoticed recently. Adding `//crates/be-host:web` (or one
target that depends on it) to verify catches that, at the cost of every web
crate's wasm build; whether that is worth it is the user's call.

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
| be-block-commit, `similar` | `MergeResult`; `similar` is the line diff under be-block-model's sequence merge (`merge_lines`) |
| be-block-store (no `vault`), sha2, ciborium | `Part`/`PartRef`/`Hash`/`Manifest`, CBOR of the model |
| be-block-session (no `store`) | the sequencer and `SessionMessage` the replica runs |
| be-block-protocol, be-block-graph | `SessionState`, `ClientId`, `RelayKind`; graph only for `Access`/`BlockParent` in the protocol, and `WORKSPACE_TYPE` |
| be-sdk-type-api, be-wasm-ops, be-wasm-guest-abi | the protocol and the guest ABI |
| be-lib-ui-keys, logicgame, rand | be-type-terminal and be-type-logic-grid's own needs |

The apps (`wasi_guest`) still build all of be-block, so be-lib-search and its
unicode and icu data build for them too, though no app indexes anything:

- be-sdk depends on be-block and re-exports it (`be_sdk::be_block`), so every
  app compiles it. Eight apps use built-ins from it (database, desktop,
  file-tree, infinite-canvas, logic-grid, settings, version-control,
  workspace), as do be-sdk's `root_settings` and be-sdk-beui's settings.
- be-lib-search is used only by be-block's `search_index` and by
  be-host-stack.

## Steps

1. Move `search_index` out of be-block into a host-only crate (for example
   `be-block-search-index`) that be-host-stack and be-block-client depend on.
   be-block then drops be-lib-search, and the apps stop building be-lib-search,
   be-lib-graphemes, caseless, unicode-normalization and the icu crates.
   Mechanical.
2. Make be-sdk depend on be-block-content only, and have the apps that use a
   built-in (the eight above, plus be-sdk-beui's settings) depend on be-block
   themselves; or move what be-sdk's `root_settings` needs into a crate the
   SDK owns. Apps that use no built-in (checklist, text, chat, image, video,
   pdf, map, the examples, ...) then stop building be-block. Mostly
   mechanical; deciding which of `root_settings` is SDK surface is the design
   part.
3. Split be-block-commit's host-only parts from what the replica uses:
   `retention` and `CommitStore` (already behind `store`) are server and
   client only. Small, and only a compile-time win, since the linker already
   drops them.
4. `similar` in type modules: replacing it means writing the line diff that
   `merge_lines` needs (Myers over slices) in be-block-model or
   be-lib-sequence. That is a design change, not a split: worth it only if
   it measurably shrinks the modules (`similar` was ~190 KB of a dev
   checklist; release drops most of it).
5. Build the web in verify (above), if the user wants it.

## Size levers for type modules, not taken

Measured on the release `:module` of be-type-checklist, for the user to
decide:

- opt-level: type modules are built at `OPT_LEVELS["app"]`
  (buck/platforms/profile.bzl), the toolchain's level in release. `s` or `z`
  for the wasm32 release build trades speed for size.
- The name section survives release builds (0.07–0.20 MB a module); a
  `--strip-debug`-style pass on `:module` in release removes it, at the cost
  of names in a guest's backtraces.
