# Text positions are bytes with no UTF-8 awareness

A text block's sequence holds bytes, and positions count bytes. Anything that
turns text into a `String` uses a lossy conversion, and the editor handles
invalid UTF-8 arriving from another peer.

## Why

- Hex mode can edit anything.
- The previous editor already handled invalid UTF-8, so this preserves
  existing behaviour.
