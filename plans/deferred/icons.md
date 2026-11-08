# Icons as blocks

Depends on: none.

beui and the app SDK are built around one icon font, Lucide: `beui_core::icons`
names its glyphs, a manifest's `icon` is a Lucide codepoint (26
`crates/*/manifest.json`), block-type descriptors carry `icon_codepoint` and
templates an icon (`be-sdk-manifest`), and a saved icon is a Lucide name
(decisions/beui_icons_are_saved_by_name.md). An editor can only use an icon
that font has. Change this as a whole, to generic icons, rather than by piecemeal
changes to one place at a time.

## Direction

- **An icon is a block and a key.** A block holds a set of icons and an icon is
  named by that block and a key within it, wherever an icon is stored or sent:
  manifests, block-type descriptors, templates, block metadata (`ICON`), and
  beui components.
- **Shared by content.** A set is named by its content hash, so several apps
  that declare the same set share one block instead of each shipping a copy.
- **Nothing icon-specific in what apps ship.** An app does not provide an
  "icon set" as such: it declares blocks by hash in general (an icon set is one
  kind of block it may declare), and an icon refers to one.
- Lucide becomes one such set, the one beui and first-party apps use.

## Open questions

- How an app declares a block by content hash: where the bytes live before
  any server holds them (in the app package?), and which organization's
  server stores them once used.
- Whether a set is one block with keyed entries, or a folder of image blocks
  keyed by name.
- The icon format: vector outlines tinted by the theme, raster masks, or both,
  and how a colour icon is told apart.
- How beui's styled components take an icon without depending on the block
  stack (beui knows no blocks): an icon value the host resolves, or a glyph
  source a beui program registers.
- How `ICON` metadata and the custom icons item in plans/deferred/settings.md
  ("Custom icons") fit: that item points metadata at an image block, which
  this would generalize.
- Offline and first paint: an icon must draw before its block is fetched.
