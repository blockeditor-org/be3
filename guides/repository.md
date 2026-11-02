# What is in the repository

The repository holds the product, the tooling around it, and separate projects
that build on it. VISION.md says why each part exists.

## Crates

`crates/` is flat: every crate is `crates/NAME/`, its Cargo package is `NAME`
(hyphens, never underscores), and its library is `//crates/NAME:NAME`, which
`//crates/NAME` abbreviates. Its other targets (`:test`, `:bin`, `:module`,
`:native` and the rest) sit beside it. A binary keeps its own name where it
differs from the package's (`block-app` is built from `be-host`, `be` from
`be-tool-cli`). The name's prefix says which family a crate belongs to:

- `be-app-*`: the apps, each a manifest.json and an entry module, staged as
  apps (`be-app-example-*` are the debug and example ones: they exercise the
  platform and are not apps a person uses).
- `be-type-*`: the block type module of the app with the same name
  (`be-type-checklist` for `be-app-checklist`), and `be-type-bundle`, every
  first-party type module by app id.
- `be-sdk-*`: what an app is written with: the protocol (`be-sdk-api`), the
  manifest, the framework-free guest library (`be-sdk`) and the beui one
  (`be-sdk-beui`), the libraries apps are linked against
  (`be-sdk-library-base` and `be-sdk-library-beui`) and the test harness.
- `be-host-*`: block-app, the program apps run in. `be-host` is the binary on
  top of its parts, each a crate of its own so they build in parallel:
  `be-host-base` (the frame host, notices, the panic guard, platform requests,
  the host's state and the watches the host publishes; no beui), `be-host-services`
  (Linux's D-Bus services; no beui), `be-host-stack` (the block stack peer,
  accounts, links, keys and app discovery; no beui), `be-host-runtime` (the
  runtime every app runs in) and `be-host-desktop` (the Linux and Windows
  desktop hosts, input remaps and programs). `be-host-testing` is the stack
  harness they test with, and `be-host-tests` the host tests.
- `be-block-*`: the block stack (guides/the_new_block_stack.md).
- `be-server-*`: the server and its account portal.
- `be-wasm-*`: running wasm guests, apps and type modules alike.
- `be-lib-*`: general libraries that know nothing of blocks, apps or beui, such
  as `be-lib-ui-types`, the vocabulary beui and the protocol share, and
  `be-lib-stable-hash`, the hash maps they use: seeded the same on every run,
  so what is built by iterating one comes out the same each time.
  be-tool-fix-rust-source (`hash_order.rs`, which lists the crates) reports a
  randomly seeded one there.
- `beui-*`: beui, and what only beui uses (`beui-reactive`, `beui-lucide`, the
  icon set, `beui-tests`, its two cores driven alike).
- `be-tool-*`: build, test and developer tools: the bootstrap
  (guides/bootstrap.md), `be-tool-verify`, `be-tool-local`, `be-tool-drive`,
  `be-tool-launcher` (lists pull requests and runs any of them on Android or the
  desktop), `be-tool-cli` (`be`: the tracker, `be sync` and terminal hosts),
  `be-tool-paint-review` (the page a pull request's summary links to for
  reviewing its paintings), and `be-tool-tls-diamond-*`, the test of how one
  module reaches another's thread-locals (guides/app_libraries.md,
  "Thread-locals").

## Separate projects

These depend on beui and the app system and live here so a change to either
updates them in the same pull request. They are not part of the core, and are
named on their own, without `be-`:

- `tabletop-*` (guides/adding_a_game.md) and the deterministic-game app.
- `logicgame` and the logic-grid app.
- The scene-3d app.

## Unrelated

`cvl2` is a compiler for a language of its own that writes Minecraft datapacks.
Nothing depends on it. It might one day write deterministic games, compiled both
to wasm and to plain English rules for playing them.
