# Dialogs are dock windows

Depends on: none.

Dialogs are windows: beui's own `Window` points at its anchor with a beak, is centred on its
owner, or blocks it, and `Dialog` (with `DialogFrame`) presents each kind of dialog as one such
window or, on a phone, a sheet; the dock's `DockDialog` is a window owned by its tab's panel. A
dialog window is not a `DockState` surface (never saved, never docked), and in an app it stays
an anchored view, so the app protocol does not change
(decisions/a_dialog_is_a_window_of_its_owner_not_of_the_dock.md,
decisions/dialogs_are_presented_by_kind.md). What is left:

## Steps

1. **Dialogs open as windows of their own where windows are hosted.** A `Window` of its own
   points at its anchor (`WindowPlacement::Over`), blocks its owner across windows (a cover in
   the owner's window) and closes while locked, but `DockDialog` and `Dialog` still pass
   `own_window=false`, so on the desktop and in space they stay overlays. Opening them as
   windows of their own needs a move by the host (`begin_move`) in place of the dragged `at`, the
   focus trapped across windows, and the dialog's beak drawn inside its own window or dropped.
2. **A confirmation blocks its frame, not a sibling.** A centred dialog with no owner blocks its
   whole window, which in an app is its own frame, and a modal `DockDialog` blocks only its tab's
   panel. A host test shows a confirmation in one editor's frame leaving a sibling frame live.

## The test

- A small form opened from a button opens next to it with a beak, moves when dragged by its
  bar, and is a sheet in the stacked mode.
- The lock screen still covers every window, a dialog placed away from its owner included, and
  a hosted dialog window closes while it is locked.
