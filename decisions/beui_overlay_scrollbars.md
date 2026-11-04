# beui scrollbars are overlays that reserve no gutter

The scrollbar is laid over the scroll's far edge, as on macOS: it takes no space from the content, shows only while it is wanted (scrolling, or the mouse near or on it) and fades out after `SCROLLBAR_LINGER`. A hidden bar takes no input, so presses reach the content.

## Why

Because no gutter is reserved, a scroll whose content grows past its viewport does not shift its content. Content that has to stay clear of the bar pads inside the scroll, which the edge-to-edge scroll rule already asks for.

## Rejected

- A bar that reserves a gutter: content would shift when it starts to overflow.
