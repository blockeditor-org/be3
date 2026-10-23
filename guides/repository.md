# What is in the repository

The repository holds the product, the tooling around it, and separate projects
that build on it. VISION.md says why each part exists.

## The product

- The block stack (`be-*`), beui (`beui*`), block-app, and the plugin API and
  SDK (`block-*`).
- The editors under `crates/editors`, except the debug and example ones below.
- The desktop: the linux-desktop editor, `be-wayland` and `beui-adapter-drm`.

## Tooling

- `paint-review` and `paint-review-web`: the web page a pull request's summary
  comment links to for reviewing the paintings it changed.
- `be-launcher`: installs CI builds on Android and lists pull requests.
- `be-cli` (`be`): reads and changes the tracker (guides/tracker.md), and keeps
  a folder and a directory on disk in step with `be-sync` (guides/sync.md).
- `ghostty-vt`: the terminal core be-launcher shows build output with, kept
  building for a terminal in the desktop session.
- The build and test tools: `buck-tools`, `local-tools`, `bootstrap`,
  `be-drive`, `app-tests`, `paint-snapshot` and the rest.

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
