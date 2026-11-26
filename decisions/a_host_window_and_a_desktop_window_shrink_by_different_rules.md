# A window in the host and a window on the displays shrink by different rules

Every floating window keeps its grip, a corner the size of its handle, on an area it can be
grabbed in, by one rule: be-lib-geometry's `Rect::grabbable`. Each caller passes its own grip
(be-sdk-window-stack's `KEPT`, be-host's window its handle's height, beui-dock-host's
floating window its bar). How a window too big for its area shrinks stays two rules:

- **be-host's own window** (`compositor/desktop.rs`, `clamped`), shown inside the host window,
  is kept wholly in it: as wide as the host at most, and shrunk from below to what is left
  under its top while that leaves at least `SHRINK_TO` of it, else moved up so it does not
  cover the opener above it.
- **A desktop window** (`be_sdk_window_stack::fitted`) shrinks only on an axis a band insets
  its work area, never past the display, and otherwise keeps its size and may hang off any
  edge; one on no display (unplugged) comes back whole onto the nearest.

## Why

- They answer different places. The host window is the whole room an app's window has, with
  nothing past its edges, and its window was opened from something it must not cover. A
  display sits beside others, and a window may hang off one edge onto another or out of
  view as a person put it, which the desktop keeps until there is room again (`wanted`).
- Taking either rule for both changes what users see: the desktop's would let a window
  opened in the host hang past it, out of reach; the host's would pull every desktop window
  wholly onto one display and shrink it there.

## Rejected

- One shrink rule in be-lib-geometry beside `grabbable`, with the desktop's winning where
  they differ (plans/one_way_in_the_host.md, step 1, before it landed): the rules share no
  case, so one function would be the two behind a flag.
