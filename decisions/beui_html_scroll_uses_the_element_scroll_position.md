# The HTML core scrolls an `Offset` through its element's scroll position

On the HTML core, an `Offset` is an `overflow: hidden` element moved through `scrollTop`/`scrollLeft`, not by a CSS transform on its content.

## Why

The browser's own scrolls (find in page, `scrollIntoView`, moving focus) change the element's scroll position, so beui adopts them as the offset instead of fighting them.

## Rejected

- Moving the content with a transform: the browser's own scrolls would not be adopted as the offset.
