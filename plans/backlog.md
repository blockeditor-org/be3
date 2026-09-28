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
