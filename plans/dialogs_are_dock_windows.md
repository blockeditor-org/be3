# Dialogs are dock windows

Depends on: none.

beui has three ways to show something over a frame: `Modal` (centred, filling or a sheet, with a
scrim), `Anchored` (a card with a beak, or a sheet on finger touch) and `Dialog`, which
rebuilds `Anchored`'s card-or-sheet switch for itself (its own finger check, "ever opened" flag,
`DialogSheetHandle` beside `AnchoredSheetHandle`, `DialogAnchor`/`DialogTarget` beside
`OverlayAnchor`, and the card placement `BELOW_CENTER.gap(4).margin(8)` defined again in
beui-dock-host and be-sdk-beui). The dock beside them already has windows that float, move,
resize, snap and hang from a tab (`DockDialog`). One concept, the window, can do all of it:
blocking its owner becomes a property of a window, not another kind of overlay.

The window is beui's own `Window`: anchoring, blocking and sheets are its properties, the dock's
dialog window (`DockDialog`) builds on it, and a `Dialog` with no dock above it is a plain `Window`
overlay. A dialog window is not a `DockState` surface (never saved, never docked), and in an app it
stays an anchored view, so the app protocol does not change
(decisions/a_dialog_is_a_window_of_its_owner_not_of_the_dock.md).

## Steps

1. **A window of its own points and blocks too.** A `Window` a host opens as its own window
   (`own_window`, where windows are hosted) opens at its `placement` and ignores `anchor`, `beak`
   and `blocks`, so `DockDialog` stays an overlay there (`own_window=false`). Place it beside its
   anchor through `WindowPlacement::Over`, and block its owner across windows, before a dialog
   may be one.
2. **Host-level layers stay overlays.** The lock screen, reauthentication and onboarding (kind
   4) are not windows: the lock screen is a security boundary and must never move, dock or be
   covered. They keep the host's lock layer and a plain full-window overlay; that is `Modal`'s
   only remaining use (with `Fullscreen` and `ModalSheet`), and it shrinks to it:
   `ModalPlacement` goes.
3. **The decisions and guides follow.** decisions/dialogs_are_presented_by_kind.md is rewritten
   for windows; guides/beui/overlays.md and guides/beui/docking.md describe one way.

## The test

- A small form opened from a button opens next to it with a beak, moves when dragged by its
  bar, and is a sheet in the stacked mode.
- A confirmation blocks presses on its frame and not on a sibling frame.
- The lock screen still covers every window, a moved dialog included.
