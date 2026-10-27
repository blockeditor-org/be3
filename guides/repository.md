# What is in the repository

The repository holds the product, the tooling around it, and separate projects
that build on it. VISION.md says why each part exists.

## The product

- The block stack (`be-*`), beui (`beui*`), block-app, and the plugin API and
  SDK (`block-*`).
- `lucide`: the icon set beui and the block catalog draw from.
- `ghostty-vt`, `terminal-emulator` and `terminal-view`: the terminal emulator's
  bindings, the emulator read back as terminal lines (the terminal host and
  be-launcher's build output), and the beui view that draws lines (the terminal
  editor and be-launcher).
- The editors under `crates/editors`, except the debug and example ones below.
- The desktop: the linux-desktop editor, `be-wayland` and `beui-platform-drm`.

## Tooling

- `paint-review` and `paint-review-web`: the web page a pull request's summary
  comment links to for reviewing the paintings it changed.
- `be-launcher`: lists pull requests and runs any of them on Android or the
  desktop, built on request by the build helper (guides/buck2.md).
- `be-cli` (`be`): reads and changes the tracker (guides/tracker.md), keeps
  a folder and a directory on disk in step with `be-sync` (guides/sync.md), and
  hosts terminal blocks (guides/terminal.md).
- `build-helper`: builds the APKs and native apps be-launcher installs, on
  request, on one host, and wakes Namespace's cluster (guides/build_server.md).
- The build and test tools: `buck-tools`, `local-tools`, `verify`, `bootstrap`,
  `be-drive`, `app-tests`, `beui-shared-tests` (beui's two cores driven alike),
  `paint-snapshot` and the rest.

## Debug and example editors

counter, triangle, pan-zoom, beui-gallery, block-debug, checklist and
pixel-ray-tracer. They exercise the platform; they are not apps.

## Separate projects

These depend on beui and the plugin system and live here so a change to either
updates them in the same pull request. They are not part of the core:

- `tabletop-games` and the deterministic-game editor.
- `logicgame` and the logic-grid editor.
- The scene-3d editor.

## Unrelated

`cvl2` is a compiler for a language of its own that writes Minecraft datapacks.
Nothing depends on it. It might one day write deterministic games, compiled both
to wasm and to plain English rules for playing them.
