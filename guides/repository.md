# What is in the repository

The product, the tooling around it, and separate projects that build on it. VISION.md says why
each part exists.

## Crates

`crates/` is flat: every crate is `crates/NAME/`, its Cargo package is `NAME` (hyphens), and its
library is `//crates/NAME:NAME`, abbreviated `//crates/NAME`, with its other targets (`:test`,
`:bin`, `:module`, `:native`, ...) beside it. A binary may have its own name (`be` is built from
`be-tool-cli`). The prefix gives the family:

- `be-app-*`: the apps, each a manifest.json and an entry module; `be-app-example-*` exercise
  the platform and aren't apps a person uses.
- `be-type-*`: the block type module of the app of the same name; `be-type-bundle` holds every
  first-party type module by app id.
- `be-sdk-*`: what apps are written with: the protocol (`be-sdk-api`), the guest libraries
  (`be-sdk`, `be-sdk-beui`), the libraries apps link against and the test harness.
- `be-host-*`: be-host, split into crates that build in parallel. `be-host-base`,
  `be-host-services` and `be-host-stack` have no beui; `be-host-runtime` runs apps;
  `be-host-desktop` holds the desktop hosts; `be-host-testing` and `be-host-tests` are its test
  harness and host tests.
- `be-block-*`: the block stack (guides/the_new_block_stack.md). `be-server-*`: the server and
  its account portal. `be-wasm-*`: running wasm guests.
- `be-lib-*`: libraries that know nothing of blocks, apps or beui. `be-lib-stable-hash` is the
  hash map to use where iteration order reaches output: be-tool-fix-rust-source
  (`hash_order.rs`) reports randomly seeded ones in the crates it lists.
- `beui-*`: beui and what only beui uses.
- `be-tool-*`: build, test and developer tools, including the bootstrap (guides/bootstrap.md),
  `be-tool-cli` (`be`), `be-tool-launcher` (runs any pull request on Android or the desktop) and
  `be-tool-paint-review` (the page for reviewing a pull request's paintings).

## Separate projects

These depend on beui and the app system and live here so a change to either updates them in the
same pull request. They are not part of the core and are named without `be-`: `tabletop-*`
(guides/adding_a_game.md) and the deterministic-game app, `logicgame` and the logic-grid app,
and the scene-3d app.

## Unrelated

`cvl2` is a compiler for its own language that writes Minecraft datapacks. Nothing depends on
it.
