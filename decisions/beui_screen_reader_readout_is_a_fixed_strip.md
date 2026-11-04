# The screen reader simulation's readout is a fixed-height strip the document does not get

The inspector's screen reader simulation shows what it says in a strip of fixed height taken off the bottom of the shown rectangle, not in a sheet drawn over the document. A long phrase is clipped rather than wrapped.

## Why

The document's height stays the same whatever the reader says. The readout sits outside the filtered region, so it stays legible however far the blur or contrast reduction is turned up.

## Rejected

- A bar that grows with the text: it would relay out the page on every utterance.
