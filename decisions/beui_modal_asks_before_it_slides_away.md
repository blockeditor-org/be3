# A modal reports dismissal at once and goes only when its owner closes it

Escape, back and a press on the scrim reach `unstyled::Modal`'s `on_dismiss` immediately; the modal goes only when its owner closes it after such a request, and stays put if the owner keeps it open. A modal sheet then slides down out of sight; every other modal (`ModalMotion::Still`) goes at once.

## Why

The default `Slides` motion used to run the slide-away first and call `on_dismiss` once the page was off screen, so Escape and a press outside closed nothing until the animation had run. Asking first lets a "discard changes?" answer, or an app's `DialogDismissal`, decide. A palette vanishes on Escape rather than sliding away.

## Later: no slide but a sheet's

Modals used to slide away in a `BackSlide` (`ModalMotion::Slides`), following a back gesture and lingering inert while they went. That motion was hard to keep right beside the owner's say, and modals are to give way to dock windows (plans/dialogs_are_dock_windows.md), so it was removed: a modal that is not a sheet disappears with no animation.
