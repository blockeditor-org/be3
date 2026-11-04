# Where a catcher hears the pointer is declared, not asked at press time

An `Interactive`'s `hit_area`, `capture_presses`, `gesture_area`, `drawing` and each `Claim` of `claims` take an `Area` (`Nowhere`, `Whole`, `Within(HitRegion)` or `CaretHandles`) that the component derives as a memo of its state, rather than a callback the core asks when a press arrives.

## Why

A core whose hit testing runs before any handler can run, the browser's on the HTML core, has to know the areas ahead of time. Declaring them keeps hit testing the same on every core.

## Rejected

- Asking the catcher at the moment of a press whether it wants it: impossible on the HTML core, where the browser decides the target before beui code runs.
