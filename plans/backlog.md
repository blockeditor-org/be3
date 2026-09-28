# Backlog

The running to-do list, grouped. Every item says what "done" looks like.
Items marked **(ask)** need a decision from the user during the work. Fixed
items are removed as they land.

## 1. Android startup crash
The app is expected to crash on startup on Android. The emulator can't run in
the cloud containers (no KVM, see guides/running_on_android.md), so this needs
a machine with KVM or a device.

## 2. beui scrolling

- **First scroll is short on Linux.** The first wheel event scrolls less than
  every later one.
- **Edge fade.** The styled scroll view fades content out with a short
  opacity gradient at clipped edges instead of a hard cut.

## 3. beui text and input

- **Multiline editor on regular text nodes.** The multiline editor lays out
  its text itself in a canvas. Rebuild it on beui text nodes, adding to beui
  whatever it is missing.
- **Emoji picker in the multiline editor.** `:` opens a menu, typing after it
  filters, Enter inserts.

## 4. beui fonts

- **Fonts.** Plugins shouldn't each embed fonts: the app provides fonts to
  plugins, falling back to system fonts natively. Tests use a fixed font list
  with no system fonts so snapshots are stable. On web the DOM renderer
  (blockeditor-org/be3#209) uses the browser's fonts; wgpu plugins on web
  still need an answer.

## 5. Tooling and repo hygiene

- **Crate folder names use `-`.** Rename every crate folder under `crates/`
  that uses `_` (`beui_macros`, `reactive_macros`, `tabletop_games` and its
  rules, the `crates/editors/*` folders).
