# A root's dialog is beui's dialog on a surface the window's size

be-root shows each of its dialogs as be-host's own UI does, beui's `DialogFrame` around a
`DialogBody`, on a surface of its own the size of the window, over its other layers: the card
over beui's scrim, or a sheet in a narrow window, which Escape and a press on the scrim dismiss
through `DialogFrame`'s `on_dismiss`. Its layer takes every press and the focus
(plans/host_as_platform_adapter.md, 4g7a2).

## Why

- It looks and behaves as be-host's dialogs do, card, scrim, sheet below `SHEET_BELOW` and
  motion included, from one component rather than a copy of their look.
- A dialog sized to its content needs a surface that knows its content's height, which
  `BeuiSurface` cannot measure; a surface the window's size, with the card laid out inside it,
  needs no measuring.
- One way to dismiss: `on_dismiss`, rather than a press the scrim layer captured for the
  root's own layer as well as Escape handled in each dialog.

## Rejected

- **A scrim layer and a dialog layer of fixed size** (step 4g4's design, which
  be-sdk-root-testing keeps as a fixture of the layer list's `captures` and placers): a card of
  a fixed size cannot fit be-host's dialogs, which differ in width and grow with their rows,
  and the look and the sheet would be copies of beui's.
- **Moving the fixture onto `DialogFrame` too**: its dialog is what be-wasm-root-host's tests
  check of `captures` and placers, which a full-window layer does not exercise.
