# Control flow swaps its child in place, and takes a host node only where a node is needed

`Show`, `Dynamic`, `Keyed` and `Loading` return a `DynamicSegment<C, N>` whose type-level
count `N` (`One`, `ZeroOrOne`, `Many`) comes from what their branches build. Where they go
decides what they cost:

- In a run (`List`, `Layers`, a text's spans), any count fills its place with no node of its
  own, so a hidden `Show` takes no room and adds no spacing.
- In a `Child` slot (`Frame`, `Interactive`, `CanvasItem`, a styled `Card`, a
  `Render<_, Child>`), exactly one fits (a `Show` with a `fallback`, a `Dynamic` building
  one), and `Option<Child>` also takes `ZeroOrOne`. The parent swaps its one child in place
  (`Single::mount`); both cores can.
- In a slot typed `NodeId`, which needs a node built once (`ListItem`, `Root`, `Offset`, a
  `ForEach` row, a component that measures or portals its child), `One` or `ZeroOrOne` is
  held by a bare frame the core builds: it sizes to its child, draws nothing and swaps the
  child in place.
- A run in a slot that takes one child is a compile error that says to put a `List` or
  `Layers` around it.

## Why

- A host node for every one-child control flow would put an empty node in every list where a
  `Show` is hidden, and a list's spacing counts that node: hundreds of `Show`s inside spaced
  `List`s would open gaps. Filling a run with no node keeps today's layout.
- Swapping in place costs no node in the common single-child slots (a `Frame`'s child, an
  `Interactive`'s), where the slot already belongs to a node that can change its child.
- The host frame keeps the one consistent shape the later control-flow spec asks for: any
  control flow that builds at most one node fits anywhere one child fits, and its cost (one
  frame, like the `Layers` or `List` wrapper written by hand before) is paid only where a
  built node is required.
- Both cores already had what this needs: a native frame's child is an `Option<NodeId>` set
  through `get_mut_as` (layout, adoption, damage and accessibility follow it), and the HTML
  core fills a run between anchors.

## Rejected

- Always returning the `NodeId` of a layout-transparent host for one-child control flow, so
  it also takes `@test_id` and `@node_ref`: it changes layout wherever a hidden `Show` sits in
  a spaced list, and costs a node in every slot. `@test_id`/`@node_ref` go on a node inside a
  branch instead.
- Two names for `Show` (one for runs, one for a single child): the branches' types decide
  where a `Show` fits, so one name serves both, with every mismatch still a compile error.
