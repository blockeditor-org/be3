# A dialog is a window of its owner, not a surface of the dock

Dialogs become windows (plans/dialogs_are_dock_windows.md), and what makes one a dialog is a
property of beui's own `Window` (`beui-view/src/window.rs`), not of the dock. Pointing and
blocking are built; sheets and the `Dialog` with no dock above it arrive with that plan's
later steps:

- **The base `Window` carries it**: pointing at an anchor with a beak, blocking its owner (no
  presses, a scrim, focus trapped and given back), and showing as a sheet. The dock's floating
  dialog window (`DockDialog`) builds on it, and a `Dialog` with no dock above it is a plain
  `Window` overlay. So be-host's own UI, a root guest, the desktop's bar and every test that opens
  a dialog with no dock keep one API.
- **A dialog window is not a `DockState` surface.** It is not saved with the layout and its body
  never becomes a tab: it cannot be docked into a pane. It moves (and later snaps) as a window
  does, and keeps where it was moved until it closes.
- **In an app it stays an anchored view** of the app's own document, as everything that escapes a
  frame is (decisions/anchored_views_for_everything_that_escapes_a_frame.md). A window that points
  at its anchor is placed `Beside` it, which escapes a window with a layer space; the app protocol
  does not change.

## Why

- Many dialogs open where no dock is above them (be-host's members and keys dialogs, a root's
  onboarding, the desktop's power menu, beui-demo). A dialog that needed a dock would need a
  second way to show it there, or a dock in every `Root`.
- A dialog is about one task of its owner and goes when the task ends; saving it, or letting its
  form become a tab, has no meaning once its owner moved on.
- Crossing the protocol as a dock window would make the host place an app's dialog body, which
  the app draws itself today with no round trip.

## Rejected

- **Every `Root` provides a `DockHost`**, so a dialog always has a dock: a dock in documents that
  have no tabs, for one feature.
- **Dialogs outside a dock stay `Modal` or `Anchored`**: two ways to show a dialog, by where it
  is opened.
- **Dialog windows as `DockState` surfaces** (dragged into panes, saved): see Why.
- **An app's dialog as a window of the host's dock** (a `DockBodies` entry): see Why.
