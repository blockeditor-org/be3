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

1. **A `Dialog` opens as a window of its own where windows are hosted**, as a `DockDialog`
   does. Tried, it broke be-host's own dialogs under its desktop host (share links, text
   settings, leaving a workspace) and the desktop's alerts: a dialog's window opened at its
   starting size, the scrim of its cover painted in the wrong place, and tests read its rects in
   its own window's points. Those need fixing first.

## The test

- A small form opened from a button opens next to it with a beak, moves when dragged by its
  bar, and is a sheet in the stacked mode.
- The lock screen still covers every window, a dialog placed away from its owner included, and
  a hosted dialog window closes while it is locked.
