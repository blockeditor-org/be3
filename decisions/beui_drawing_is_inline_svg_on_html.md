# A beui Drawing is inline SVG on the HTML core

On the HTML core a `Drawing`'s callback paints into an SVG `Painter`. `Picture` and the scrollbar thumb stay `Drawing`s on both cores (one way to paint an image or a thumb): an SVG `<image>` and `<rect>` on HTML. The colour wheel's GPU face is computed on the CPU into an image there.

## Why

A server-rendered page shows drawings before wasm loads, which a `<canvas>` could not.

## Rejected

- `<canvas>`: draws nothing until wasm runs.
- Pink placeholder rectangles, as the first version drew.
