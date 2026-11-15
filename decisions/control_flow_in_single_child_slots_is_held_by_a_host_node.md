# Control flow in single-child slots: a host node, not a swap in place

`Show`, `Dynamic`, `Keyed` and `Loading` take a `fallback`. Each returns a lazy form whose
type carries how many children it builds: `OneOf<C>` (exactly one), `ZeroOrOne<C>` (zero or
one) or a run (`DynamicSegment<C>`). The count comes from what each branch builds
(`Counted`), and the form from the count (`Count::Form`).

- Among a parent's run it mounts as a dynamic segment with no extra node, as before.
- In a slot that takes one node it becomes a host node whose single child the control flow
  swaps. Natively the host is a bare `Frame`; on the HTML core it is a `display: contents`
  element that forwards its placed styles (flex, min and max, alignment, grid area) to the
  child it holds, and moves them across on a swap. The host also carries `@test_id` and
  `@node_ref` (`NamesNode`), and control flow named that way becomes a host even in a run.
- A run in a slot that takes one node, and a `Show` without a `fallback` where a node must
  always be, are compile errors that say what to add.

## Why

- Every single-child slot keeps one type, `Child = NodeId`, and takes one-child control flow:
  the slots that measure, place or portal their child included.
- Control flow can be named with `@test_id` and `@node_ref`.
- Fraction widths and other placed styles reach the child on HTML, which a `<Layers>`
  wrapper got wrong. (A wrapper that needs a box of its own, such as `Interactive`, builds
  its own element around a host, since a `display: contents` element has none.)
- The node a host costs is the one a hand-written wrapper cost before, and only where one
  node is needed; a run still adds none.

## Rejected

- Swapping in place inside the parent (each single-child node kind replaces its child). It
  saved one node, but it needed a second single-child slot type besides `NodeId`; it left
  about a third of single-child slots (those that measure, place or portal their child) still
  refusing control flow; it could not name control flow with `@test_id`.
- A one-child `Layers` as the host: it does not pass scroll anchoring through, and is not
  transparent to fraction widths on HTML.
- Single-child slots typed `Prop<NodeId>`: a `Prop` is a value read reactively and owns no
  branch scope, so it cannot hold `keep_alive` or `Loading`'s paused branches without
  becoming the swap host under another name, and slots that need the id up front would still
  stay `NodeId`.
- The count as an associated type of `IntoSegment<C>`: the form has to be chosen before the
  parent's child type `C` is known (the parent is found through the form), and a node that
  converts into several child types (`NodeId`, any `Placed<P>`) would leave the count
  unresolved.
