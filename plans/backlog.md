# Backlog

The running to-do list, grouped. Every item says what "done" looks like.
Items marked **(ask)** need a decision from the user during the work. Fixed
items are removed as they land.

## 1. Broken things

### 1.1 Scene 3D (`crates/editors/scene_3d`)
- Mouse look changes the camera but does not request a repaint.
- Holding W repaints, but the plugin flickers between a correct frame and a
  blank one.

### 1.2 Android startup crash
The app is expected to crash on startup on Android. The emulator can't run in
the cloud containers (no KVM, see guides/running_on_android.md), so this needs
a machine with KVM or a device.

## 2. beui scrolling

- **First scroll is short on Linux.** The first wheel event scrolls less than
  every later one.
- **Edge fade.** The styled scroll view fades content out with a short
  opacity gradient at clipped edges instead of a hard cut.

## 3. beui rendering and damage

- **Inspector damage rect.** `./scripts/buck run //crates/beui:dock-example`:
  narrow the window until the inspector becomes a tab, open it, click "app",
  click "inspector" — the second time the damage rect is off and the
  inspector does not paint to the screen edge.
- **Bad text rendering.** "No file chosen" beside "Choose file…", and the
  background-fill demo window's text in dock-example.
- **1px hover overlap.** be-launcher's PR list: two rows can be hovered at
  once. Find the cause and fix it in beui.
- **Over-repaint detection.** A debug mode that diffs each frame's output
  against the previous one and reports damage much larger than the changed
  pixels (and changed pixels outside damage). Damage is decided at layout
  time, so painting can skip what is outside it.
- **E-ink theme corners.** 1px rounded corners look lighter than the straight
  edges they join.

## 4. beui text and input

- **Ellipsis.** Single-line text that truncates with "…". First user:
  be-launcher's toolbar ("Checked out … commit info"), which overflows onto
  the tab buttons at narrow widths.
- **Selectable text across nodes**, with copy. First user: be-launcher's PR
  tab.
- **Multiline editor on regular text nodes.** The multiline editor lays out
  its text itself in a canvas. Rebuild it on beui text nodes, adding to beui
  whatever it is missing.
- **Emoji picker in the multiline editor.** `:` opens a menu, typing after it
  filters, Enter inserts.
- **Tab escapes the multiline editor** when the editor consumes Tab.
- **File dialog fails silently.** Without xdg-desktop-portal or zenity,
  "Choose file…" does nothing; show an error saying what to install.

## 5. beui fonts

- **Fonts.** Plugins shouldn't each embed fonts: the app provides fonts to
  plugins, falling back to system fonts natively. Tests use a fixed font list
  with no system fonts so snapshots are stable. On web the DOM renderer
  (blockeditor-org/be3#209) uses the browser's fonts; wgpu plugins on web
  still need an answer.

## 6. Tooling and repo hygiene

- **Crate folder names use `-`.** Rename every crate folder under `crates/`
  that uses `_` (`beui_macros`, `reactive_macros`, `tabletop_games` and its
  rules, the `crates/editors/*` folders).
