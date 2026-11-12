# Backlog: set aside

Depends on: none.

Items set aside until the user picks them up.

## Block size

- **A text block's reported size grows fast.** long1.md, about 30 KB of Markdown, showed as
  "Text · 31 KB" in the files list after import and "Text · 154 KB" after a handful of short
  edits and a few restarts; tiny.md, with about 21 KB pasted in, showed 22 KB. Likely revision
  history counted in the size, or kept more than it needs to be.

## beui scrolling

- **First scroll is short on Linux.** The first wheel event scrolls less than
  every later one. No reproduction yet, and a headless machine has no real
  wheel: it needs the backend (winit, DRM and libinput, or the web) and an
  input sequence to replay before it can be fixed.
