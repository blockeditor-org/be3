# Fingers on a drawing canvas are turned off by a switch, not palm rejection

While the `canvas_touch` preference is `CanvasTouch::Off`, a finger that lands on an `Interactive` with `drawing` reaches nothing (no press, drag, long press, scroll or pinch, there or around it) until it lifts; pens, mice, touchpads and fingers elsewhere are unaffected. The person turns the switch.

## Why

A switch the person sets behaves predictably; palm rejection guesses.

## Rejected

- Palm rejection: it guesses which touches are palms.
