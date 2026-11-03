# A modal reports dismissal at once and slides away only when its owner closes it

Escape, back and a press on the scrim reach `unstyled::Modal`'s `on_dismiss` immediately; the modal slides away only when its owner closes it after such a request, lingering inert while it goes, and stays put if the owner keeps it open (`BackSlide`'s `open` and `on_gone`). The command palette uses `ModalMotion::Still` by choice.

## Why

The default `Slides` motion used to run the slide-away first and call `on_dismiss` once the page was off screen, so Escape and a press outside closed nothing until the animation had run. Asking first lets a "discard changes?" answer, or an app's `DialogDismissal`, decide. A palette vanishes on Escape rather than sliding away.
