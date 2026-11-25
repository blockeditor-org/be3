# One Dialog, presented by its kind and size, as a window

There is one `Dialog` (with `DialogFrame`, the same around a body of the caller's own), and it
shows as a beui `Window` (decisions/a_dialog_is_a_window_of_its_owner_not_of_the_dock.md). How it
is presented follows from what it is:

1. Small per-object forms opened from a control (text settings, file-tree Inspect, a database
   filter, canvas Export, Rename): a window beside the control with a beak
   (`WindowAnchor::Node`, or `Beside` a rect), and a sheet on phones. It escapes the frame, has
   no scrim, closes on an outside press, blocks presses on the owning frame, traps focus and
   gives it back.
2. Confirmations: a window centred over the frame that asked (`WindowAnchor::Centre`), with a
   scrim, inside its region. Never a sheet.
3. Large forms and embedded editors (block creation, Share, Members, Contacts, conflict
   compare): a window centred on the owning frame and blocking it; a full-height sheet on phones.
4. Host-level layers with no origin. The lock screen is no window: it is a security boundary
   that never moves, docks or is covered, a full-window `Modal` at `LayerLevel::Lock`.
   Reauthentication, onboarding, adding a device and sync errors are confirmations or large
   forms of the host window, centred on it.
5. Program dialogs (Wayland and X): windows centred on their parent.
6. The command palette: a blocking window at the top centre of its window (`WindowAnchor::Top`).

A dialog hung from a dock tab (`DockDialog`) is the same window owned by its tab's panel: beside
the tab, and a sheet in the dock's stacked and gathered modes.

A small dialog points at the control that opened it; if that was a menu row that has closed, at
what the menu was opened on; if the command came from a menu another view drew, a shortcut or the
palette, at the editor's tab (`use_frame_anchor`); and if none is visible, at the top centre of
the owning frame (`WindowAnchor::Top`), where it draws no beak.

## Why

- A small dialog is always one window anchored to a rect or node, so no second way of hanging it
  from a frame is needed.
- The beak is drawn only when the anchor is something the user can see.
- One concept, the window, holds every presentation: blocking its owner is a property of a
  window, not another kind of overlay.

## Rejected

- `HangFrom` and `DialogAnchor::Frame`: replaced by the anchor fallback above.
- A `Modal` beside an anchored `Overlay` inside `Dialog`, with a `ModalPlacement` per kind: one
  `Window` with an anchor does both.
