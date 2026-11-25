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

Steps 1 (a `Window` points at its anchor), 2 (it may block its owner; `DockDialog` is one) and 3
(the dock shows a dialog as a sheet in the stacked and gathered modes) are built. A window that
is a window of its own (a host that opens windows) does not point at its anchor yet; `DockDialog`
stays an overlay there.

4. **Dialogs become windows.** By the kinds of decisions/dialogs_are_presented_by_kind.md:
   - Small forms (1): an anchored window with a beak.
   - Confirmations (2): a window centred on its frame that blocks it.
   - Large forms and embedded editors (3): a window attached to its frame, blocking it.
   - Program dialogs (5): already windows.
   - The command palette (6): a borderless top-centred window.
   `Dialog`, its sheet handle and anchor types, `DialogCard`'s placement copies and
   `ModalPlacement` go; `DialogBody` stays the inside of these windows, and
   decisions/app_dialogs_are_drawn_by_the_app_with_dialog_body.md holds for an app's.
   decisions/beui_modal_asks_before_it_slides_away.md holds for a blocking window: dismissal is
   reported at once and the window goes only when its owner closes it.
5. **Host-level layers stay overlays.** The lock screen, reauthentication and onboarding (kind
   4) are not windows: the lock screen is a security boundary and must never move, dock or be
   covered. They keep the host's lock layer and a plain full-window overlay; that is `Modal`'s
   only remaining use, and it shrinks to it.
6. **The decisions and guides follow.** decisions/dialogs_are_presented_by_kind.md is rewritten
   for windows; guides/beui/overlays.md and guides/beui/docking.md describe one way.

## The test

- A small form opened from a button opens next to it with a beak, moves when dragged by its
  bar, and is a sheet in the stacked mode.
- A confirmation blocks presses on its frame and not on a sibling frame.
- The lock screen still covers every window, a moved dialog included.
