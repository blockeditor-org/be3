# Backlog: set aside

Depends on: none.

Items set aside until the user picks them up.

## beui scrolling

- **First scroll is short on Linux.** The first wheel event scrolls less than
  every later one. No reproduction yet, and a headless machine has no real
  wheel: it needs the backend (winit, DRM and libinput, or the web) and an
  input sequence to replay before it can be fixed.
