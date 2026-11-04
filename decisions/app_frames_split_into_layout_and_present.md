# An app's frame is a layout commit and then a present

An app answers a host draw in two commits with the same serial: a layout commit (sizes, frame reports, window requests, child placements, docks) sent through the host's `laid-out` call as soon as it has laid out, and then a present commit once it has painted. The host applies the layout commit at once, so a child its placer resized gets its new screens and is asked to draw in the same host frame, while the placer paints. A host frame runs in rounds until no commit changes what it lays out, waiting at most until the display's next present. A view whose frame arrives at a size it was not laid out at is drawn at its own size from the top left, cropped, never scaled.

## Why

- When input, layout and painting ran as one step, a parent's layout reached the host only together with its finished painting, too late for a child to paint in the same pass, so the child lagged one or two frames and was shown stretched or misplaced.
- The phases are framework-neutral ("apply input and lay out", "paint what you laid out"), so the protocol stays independent of beui; an app that does not split its work sends an empty layout and does everything when asked to present.

## Rejected

- Scaling the old frame of a child that missed the frame: that is the stretching this replaced.
- Holding a parent's frame back for a slow child: it would make the whole dock stutter, and showing the child's last frame unscaled for a frame or two is good enough.
