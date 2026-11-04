# A beui Break copies without a newline

Copying text across a `Break` must not put a newline in the clipboard, on every browser engine as well as natively.

## Why

It is a requirement of the HTML core, where the browser serializes the selection itself.

## Rejected

- `display: table` on an empty span: selection serializes block-level boxes with newlines, so it probably fails.

The first candidate is an empty element whose `::after` is `content: "\A"; white-space: pre`, since generated content is neither selected nor copied; if that does not hold in every engine, a `copy` handler writes the clipboard text from the model. A Playwright test across Chromium, Firefox and WebKit decides.
