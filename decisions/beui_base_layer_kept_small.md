# The beui base layer stays small

The base layer is a fixed, small set of nodes (`Frame`, `List`, `Spacer`, `Layers`, `Grid`, `Text`, `Offset`, `VirtualList`, `Canvas`, `Drawing`, `Overlay`, `Interactive`, `Portal`, `Root`, `Selectable`). New behavior is added as unstyled components composed from them, and a new concern that fits `Frame`'s single-child box model extends `Frame` rather than adding another pass-through node.

## Why

Most things are compositions of the existing nodes (`unstyled::Picture` is a `Drawing` with a size, `Window` an `Overlay` with a `window` spec, `TextArea` and `TextInput` compositions of `Text` and `Interactive`), so they behave the same on every core. A base node is justified only by a primitive the retained tree lacks: a new way to lay out, paint or receive input, as `Portal` (showing a subtree it does not own) was. `Selectable` stays a base node though it has one child: each core selects text its own way, natively over the document and on HTML with the browser's own selection (its handles, menu and find in page), which an unstyled composition would give up.

## Rejected

- A base node for each new widget or effect: most are expressible as arrangements of existing nodes, and every base node needs a body per core.
- A separate pass-through node per single-child concern: `Frame` already carries one prop per concern on one retained node.
