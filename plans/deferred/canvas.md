# Canvas

Depends on: plans/deferred/public_release.md (Web offline), for step 1 only.

A plan to make the infinite canvas (`crates/be-app-infinite-canvas`, model in
`crates/be-type-infinite-canvas`) a general 2D vector and raster image editor,
starting with a prototyping profile as quick to reach for as Excalidraw. Each
step says what "done" looks like. Items marked **(ask)** need a decision from
the user during the work.

## Where it stands

Steps 1 to 4 of the first version of this plan are built: the path shape,
camera transforms and per-entity signals in beui, and their benchmarks
(`//crates/be-app-infinite-canvas:example-canvas-bench`, guides/performance.md);
the model (one `Entity` per shape, `CanvasShape`, bound line ends, style field
by field, the hand-drawn look, freehand with pressures, rich text); the
prototyping profile (style popover, tool shortcuts and lock, text and labels
edited in place, eraser, a dot grid setting, lasso, entering groups, bending
lines, copy and export as PNG and SVG); and input on every device (pen, eraser
end and barrel button, the Ignore touch toggle, long press, coalesced and
predicted pointer events on the web, trackpad pan and pinch). The Vector and
Raster profiles exist too: a viewer picks a profile from the mode dock
(`root/mode.rs`), and a tool belongs to profiles (`Tool::belongs_to`). Raster
layers (`CanvasShape::Raster`, `crates/be-type-infinite-canvas/src/raster.rs`)
replace the pixel-art editor. Snapping exists only for angles and, as a
per-viewer choice kept per profile, to whole pixels (`CanvasState::snaps`).
decisions/canvas_profile_belongs_to_the_viewer.md,
decisions/canvas_prototyping_has_no_snapping.md,
decisions/touch_off_toggle_instead_of_palm_rejection.md and
decisions/canvas_raster_layers_replace_the_pixel_art_editor.md record why.
guides/canvas.md describes what is built.

The canvas is not a frozen format yet (guides/frozen_formats.md), so its model
can change freely until step 5.

## 1. As quick to open as a URL

- Visiting the host on the web with no account opens a blank canvas at once, kept in a
  local-only workspace in the browser (its origin private file system; see
  `plans/deferred/public_release.md`, Web offline). Local-only workspaces are permanent, not a testing aid.
- Signing in later can move the canvas into a remote workspace and share it by
  link.
- Load only the canvas app to start, and let the host install to a home
  screen. A service worker already keeps the host so it opens offline
  (`crates/be-host/web/sw.js`).
- A cold-start target on a mid-range phone, measured in CI. **(ask)** the
  target.

## 2. Precision and reuse

- Snapping to other shapes' edges and centres, with guides and equal-spacing
  hints. Off in the prototyping profile unless a viewer turns it on; on in the
  more precise profiles. Today only angles and whole pixels snap.
- A library of reusable shapes. First build what makes it easy: per-workspace
  settings, and a good way for an editor to provide data. **(ask)** whether a
  library is a block.

## 3. The other profiles

Vector and Raster exist. Next is Interactivity: a 2D level editor, where
entities carry components (they can already, through `CanvasComponent`) and a
live preview runs modular wasm behaviour. It gets its own plan when it starts.

## 4. A new text box written on its first keystroke

The Text tool adds its entity at once, in an undo group that its typing joins;
a box left empty when editing ends is cancelled. Closing the canvas while one
is still empty keeps it (the host commits an undo group its editor leaves
open), an invisible empty Text entity as a step (plans/backlog.md, Canvas).
Draft it locally instead: a draft entity of the canvas's own, merged into
`Entities` (its ids, `all`, `among`, `near_untracked`, the catalog and region,
drawing and hit testing) and edited in place like a written one, written with
its first keystroke (keys, IME composition, a paste) as the group's first edit.
Done: the Text tool, a click and the editor detaching leave no entity; typing
into the draft undoes as one step with its box; peers see nothing until the
first keystroke.

## 5. Freeze the format

Once the model and the prototyping profile have settled, add the canvas to guides/frozen_formats.md
with its tests.
