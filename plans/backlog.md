# Backlog

The running to-do list, grouped. Every item says what "done" looks like.
Items marked **(ask)** need a decision from the user during the work. Fixed
items are removed as they land.

## 1. Broken things

### 1.1 Map editor never loads (`crates/editors/map`)
Dark before refresh, never shows tiles; after pressing refresh the plugin is
a white screen. Done: tiles load and refresh works, with a test that loads a
tile from a fake tile source.

### 1.2 PDF on wasm (`crates/editors/pdf`)
`render.rs` only uses pdfium off wasm; on wasm it falls through to
`render/unsupported.rs`, and every plugin is wasm now, so PDF never renders.
Link a pdfium wasm build into the plugin via `third-party/pdfium`.
Candidates:
- https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F8057/pdfium-wasm.tgz
- https://github.com/paulocoutinhox/pdfium-lib/releases/download/8046d/wasm.zip

Both are Emscripten builds; find out whether either ships a static library
that links into a `wasm32-wasip1` module. If neither does, build pdfium for
wasi ourselves. Then delete the native path. Done: a PDF renders in the app
and a snapshot test covers a page.

### 1.3 Scene 3D (`crates/editors/scene_3d`)
- Mouse look changes the camera but does not request a repaint.
- Holding W repaints, but the plugin flickers between a correct frame and a
  blank one.

### 1.4 Games embedded in an infinite canvas
Clicking an embedded game should let it take over: the game keeps rendering
inside the canvas at its position, and the sidebars become the game's
sidebars. This no longer happens, probably since the canvas changes.

### 1.5 Android startup crash
The app is expected to crash on startup on Android. The emulator can't run in
the cloud containers (no KVM, see guides/running_on_android.md), so this needs
a machine with KVM or a device.

## 2. Data layer and server correctness

### 2.1 Access checks are quadratic
`be-server/src/blocks.rs`: `effective_access` builds a `Visibility` — the
whole `graph.access_map(account)` — per call, so any loop over blocks is
O(n²). Build it once per request and pass it down.

### 2.2 `BlockSource` duplicates `BlockParent`
`block-editor-plugin/src/host.rs` has `BlockSource { Root, Orphaned, Block }`
next to `graph.rs`'s `BlockParent { Detached, Root, Block }`. Delete
`BlockSource`.

### 2.3 Clearing a name keeps the old one
Clearing a block's name leaves the old name showing until an editor next
derives one. The clear should take effect immediately.

### 2.4 One-frame lag on writes through a block projection
An editor's write through a projection lands after that frame's pump, so
the change paints a frame late; tests need an extra `run()` and
`RelatedBlock` needs ~3 frames to settle a reference chain from cold.
Confirm it is still true; if so, make a write and its consequences paint in
the same frame, and delete the extra `run()`/`settle` calls in tests.

### 2.5 `SeedContent` from a plugin
`block-app/src/plugin_host/instances.rs` checks only that the content type is
known, not that the instance holds the block (the `Show` handler beside it
checks `entry.holds`). The server enforces edit rights, but the host should
check too.

## 3. Push, don't poll

- `watch_blocks` checks for graph changes at the start of each frame; make
  graph changes push to their watchers.
- The block top bar polls undo/redo availability and the name.
- The file tree polls the focused block, the "reveal the shown block"
  arrow, and drag-and-drop landing.
- A frame clock tests can advance: springs (window bounce), scroll momentum,
  long press and timers read `Instant::now()`. Route them through a clock on
  the context that `BeuiTest` can advance, with tests for the window bounce
  and a scroll fling.

## 4. beui scrolling

- **First scroll is short on Linux.** The first wheel event scrolls less than
  every later one.
- **Scroll latching.** Repro in the demo: scroll to the bottom, scroll down
  outside the inner scroll view, move into it, scroll down again — it stays
  latched to the outer one. Latching should end when the pointer moves.
- **Tap to stop.** A tap during a fling stops it and is not a click.
- **Overscroll on the scrollbar.** The thumb squishes while overscrolled.
- **Trackpad inertia on Wayland.** libinput gives no inertia; feed trackpad
  deltas into the same momentum the touch fling uses, started on the scroll
  stop event.
- **Android fling curve.** Match Android's `OverScroller` deceleration.
- **Edge fade.** The styled scroll view fades content out with a short
  opacity gradient at clipped edges instead of a hard cut.
- **Scroll blit with virtual lists.** When a virtualized item deletes itself
  (be-launcher's targets list) the whole viewport repaints. Damage should be
  the moved region and the gap, with the rows below moved by a copy.

## 5. beui rendering and damage

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

## 6. beui text and input

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

## 7. beui fonts, window manager, inspector

- **Fonts.** Plugins shouldn't each embed fonts: the app provides fonts to
  plugins, falling back to system fonts natively. Tests use a fixed font list
  with no system fonts so snapshots are stable. On web the DOM renderer
  (blockeditor-org/be3#209) uses the browser's fonts; wgpu plugins on web
  still need an answer.
- **Dock tab drop indicator** sits centered in the gap between two tabs, not
  on the left edge of a tab.
- **Middle click** closes a dock tab.
- **Empty dock.** The dock supports having no tabs and shows a
  caller-provided view; workspace_ui drops its unclosable "workspace" tab.
- **Inspector close button** goes to the right of the tabs in narrow
  (tabbed) mode.

## 8. Editors

### 8.1 Infinite canvas
- **Select by default, two-finger pan.** Revert the separate default pan
  tool: the select tool is chosen by default, and panning takes two fingers
  on touch (and the usual wheel/middle-drag on desktop).
- **Artboards** replace the current viewport:
  - A canvas has none by default. A new artboard tool adds one.
  - With at least one artboard, the artboard gets the normal background and
    everything outside it is darker.
  - Embedding a canvas shows its first artboard. Picking another waits for a
    future embed-options system.
- **Layers panel.**
- **Narrow layout.** Passable since the last round, but still poor; improve
  it.

### 8.2 File tree
- Right click → Inspect: the modal runs off the screen at a normal file tree
  width.
- Right click → Export: save a block to a regular file.

## 9. Tooling and repo hygiene

- **Crate folder names use `-`.** Rename every crate folder under `crates/`
  that uses `_` (`beui_macros`, `reactive_macros`, `tabletop_games` and its
  rules, the `crates/editors/*` folders).
