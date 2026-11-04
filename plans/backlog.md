# Backlog

The running to-do list, grouped. Every item says what "done" looks like.
Items marked **(ask)** need a decision from the user during the work. Fixed
items are removed as they land.

## 1. Push, don't poll

Each piece of host state the editor mirrors already notifies its own
subscribers (`Mirror` in `be-sdk-beui/src/editor.rs`), but `Editor::begin_frame`
still compares the host's change counter (`host().changes()`) every frame
before syncing the mirror. Make it fully push-based, so a frame does no
checking at all.

## 2. beui scrolling

- **First scroll is short on Linux.** The first wheel event scrolls less than
  every later one.

## 3. Editor icons in the manifest

A manifest's `icon` is a Lucide icon font codepoint (`""`), so an editor
can only use an icon from the font every beui context carries. Let an editor
declare any icon: the manifest carries the icon as vector outlines, which the
host draws with beui's path shape (`beui::Path`) and tints with
the theme. An editor that uses a `beui::icons` icon names it in its source, and
the build writes that glyph's outlines into the manifest. **(ask)** vector
outlines, or icons rasterized at build time at a few sizes. Done: no manifest
holds a font codepoint.

## 4. Anchored views

- **Bar drops across layers.** A joined dock bar dropped across layers only
  reorders within its own layer. Done: it can be dropped on any edge of any
  joined layer's pane.
- **Children over anchored views on the web.** A child placed Above the pixels
  of an anchored view shows under them. Done: it shows above them, as natively.
