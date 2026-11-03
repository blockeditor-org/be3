# On the HTML core a Drawing paints into inline SVG

On the HTML core, a `Drawing`'s `Draw` callback runs into an SVG `Painter` (`beui-core-html`'s `drawing.rs`), and the result is set as the markup of an inline `<svg>` in the drawing's element, rebuilt only when the callback, its measured size or a colour it resolved changes.

## Why

A server-rendered page shows SVG before any wasm loads.

## Rejected

- `<canvas>`: a server-rendered page could not show it before wasm loads.
