# Kept content is the caller's choice, and a sheet scrolls with Scroll

Depends on: none.

Two places in beui where a component does by hand, and differently each time, what a base
piece should do once, plus small duplicates found beside them.

## Steps

Steps 1 (a kept subtree reads the context of the `Kept` showing it), 2 (overlays build their
body on each open; keeping it is the caller's `Kept`), 3 (`Scroll`'s `snaps`) and 4 (a sheet is
one `Scroll`, with sticky items and `bar_from`, running beui's own motion on both cores) and 5
(one `Card`, with a maximum width and the frame a beak points from, under popovers and dialogs)
have landed, and so has 7 (one `find_text` and the dock fixtures in beui-host's
`native::testing`). Step 6 (one colour wheel) was measured and set aside: the cached image does not hold
a frame (decisions/the_colour_wheel_draws_on_the_gpu_where_it_can.md).
