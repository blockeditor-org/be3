# beui list children are placed by a wrapping tag

How long a child of a `List` or `Stack` is along the list is written as a tag around it, `<ListItem grow=1.0><Text>..</Text></ListItem>`, not as a modifier on the child. `ListItem` takes `grow` (a weight in the length the other children leave), `length` (fixed), `min`, `max`, `shrink`, `align` and `gap`. It builds a `ListItem` value, the child a list takes, so one written anywhere else is a compile error. `<Spacer />` is an empty `ListItem` that grows. `GridCell` places a grid child the same way.

## Why

- A modifier (`@test_id`, `@node_ref`) works on the node and keeps its type. Sizing is data the parent keeps beside the child and changes what the child is, so writing it among the node's attributes made it read as a property of the node. Agents then built nodes bare and sized them where they were placed, `let a = view! { <A /> }; view! { {a} @sizing=.. }`, not realising the tag form existed.
- A tag that encloses its child shows exactly what it applies to. A prefix (`[sizing=..] <A />`) or a trailing form (`{a} @sizing=..`) has to be read as binding to one neighbour.
- The parent's data stays typed: a node does not carry a sizing that only a list reads, so sizing a child no list will hold still fails to compile.
- Most sized children (about three in four) are the one child that takes what its siblings leave. `ItemSize::Percent(100.0)` said that as a percentage that was really a weight; `grow=1.0` says it as one.

## Rejected

- `@sizing=ItemSize::..` as a modifier: the confusion above.
- Storing sizing on every node, as CSS keeps `flex` on the item: most nodes are never a list child, and a sizing given to a node outside a list would be silently ignored instead of a compile error.
- A prefix such as `[sizing=..] <A />`: what it applies to is not obvious when another child follows.
- A `fill` shorthand or `<Fill>` for `grow=1.0`, and a list prop that makes its last child grow: a second way to write the same thing, and the second hides a choice that belongs on the child.
