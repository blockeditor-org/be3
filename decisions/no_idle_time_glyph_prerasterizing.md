# Glyphs are rasterized when a page first needs them

beui rasterizes glyphs on demand; it does not rasterize a character set ahead of time.

## Why

Rasterizing ahead of time hides the cost of a page's first open rather than removing it.

## Rejected

- Rasterizing ASCII in idle time, so a page's first open finds its glyphs ready: it helped,
  but works around the cost. Kept in reserve for when nothing better is left.
