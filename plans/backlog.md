# Backlog

The running to-do list, grouped. Every item says what "done" looks like.
Items marked **(ask)** need a decision from the user during the work. Fixed
items are removed as they land.

## 1. Push, don't poll

Editors are woken only when the host pushes a change, but each frame an
editor still compares the host's change counter before re-reading host state
and pumping projections (`block-editor-beui/src/editor.rs`). Make it fully
push-based: each piece of host state (projections, the block graph, focus,
drag, files, chrome, editable, presenting) notifies its own subscribers, so a
frame does no checking at all.

## 2. beui scrolling

- **First scroll is short on Linux.** The first wheel event scrolls less than
  every later one.
- **Edge fade.** The styled scroll view fades content out with a short
  opacity gradient at clipped edges instead of a hard cut.

## 3. Editor icons in the manifest

A manifest's `icon` is a Lucide icon font codepoint (`""`), so an editor
can only use an icon from the font every beui context carries. Let an editor
declare any icon: the manifest carries the icon as vector outlines, which the
host draws with beui's path shape (`plans/canvas.md`, step 1) and tints with
the theme. An editor that uses a `beui::icons` icon names it in its source, and
the build writes that glyph's outlines into the manifest. **(ask)** vector
outlines, or icons rasterized at build time at a few sizes. Done: no manifest
holds a font codepoint.
