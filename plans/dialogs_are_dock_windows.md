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

1. **A window of its own points and blocks too.** A `Window` a host opens as its own window
   (`own_window`, where windows are hosted) opens at its `placement` and ignores `anchor`, `beak`
   and `blocks`, so `DockDialog` stays an overlay there (`own_window=false`). Place it beside its
   anchor through `WindowPlacement::Over`, and block its owner across windows, before a dialog
   may be one.
2. **A confirmation blocks its frame, not a sibling.** A centred dialog with no owner blocks its
   whole window, which in an app is its own frame, and a modal `DockDialog` blocks only its tab's
   panel. A host test shows a confirmation in one editor's frame leaving a sibling frame live.

## The test

- A small form opened from a button opens next to it with a beak, moves when dragged by its
  bar, and is a sheet in the stacked mode.
- The lock screen still covers every window, a moved dialog included.
