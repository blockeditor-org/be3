# An app opened as a dialog draws its whole inside, and hears dismiss first

An app opened as a dialog gets the whole area inside the host's frame, with no padding and no shell title bar; the host owns placement, scrim, presence, the card or sheet frame, back and dismiss. A beui app builds its contents with the same styled `DialogBody` beui uses (be-sdk-beui hands it a `DialogLayout` and an `unstyled::DialogDismissal`). Escape, back and a press on the scrim reach the app, whose `DialogBody` runs its Cancel action and asks the host to close (`shell-message.close-dialog`). Whether the app answers is decided up front: it says `handles-dismiss(true)` while a `DialogBody` is mounted, and otherwise the host answers `dismiss-child` with `dismissed` at once. The host passes a `dialog-frame.bottom-inset`. Window dialogs keep a frame but no title bar, and report no drag region.

## Why

The app defines its own buttons with the code beui uses for its own dialogs, so beui apps match the shell's look exactly, while the protocol carries only what any framework needs and stays free of beui (an egui app gets the same area and messages and draws its own version). Dismiss goes to the app first so a form with unsaved changes can ask before closing; deciding up front avoids waiting for an answer. The bottom inset keeps the pinned footer visible above the on-screen keyboard or a gesture bar.

## Rejected

- A drag region for moving window dialogs by their title row: nothing moves dialogs today (no editor is shown in a window-presentation `DockDialog`, and dock dialogs are anchored, not dragged).
