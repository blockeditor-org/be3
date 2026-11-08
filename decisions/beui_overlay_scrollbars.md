# beui scrollbars are overlays that reserve no gutter

The scrollbar is laid over the scroll's far edge, as on macOS: it takes no space from the content, shows only while it is wanted (scrolling, or the mouse near or on it) and fades out after `SCROLLBAR_LINGER`. A hidden bar takes no input, so presses reach the content.

## Why

Because no gutter is reserved, a scroll whose content grows past its viewport does not shift its content. Content that has to stay clear of the bar pads inside the scroll, which the edge-to-edge scroll rule already asks for.

## Exception: the HTML core

The HTML core scrolls with browser scroll elements
(decisions/beui_html_scroll_uses_the_element_scroll_position.md) and shows the
browser's scrollbar, themed with `scrollbar-color`, which reserves a gutter in
Chromium's classic scrollbars. That is accepted for now: content there can
shift when it starts to overflow, and some layouts (dock and window-manager
tab bars) look off, to be dealt with later rather than by drawing beui's bar
over a hidden browser one.

## Rejected

- A bar that reserves a gutter: content would shift when it starts to overflow.
