# Reorder stays, built on drop sites

`Reorder` is kept as the one component for a list whose items are carried into a new order
or between lists. It is built on the drag primitives (`use_drop_site`, `slots_in`,
`Interactive`'s `drag`), and a caller that wants a list of carried rows uses it rather than
those primitives directly.

## Why

The plan said to remove `Reorder` if callers were as short without it. They are not. A plain
drop site over a list (beui-demo's `PlacesList`, about 50 lines) reorders within one list
and nothing more. What `Reorder` adds, in its 1,100 lines, is what every real caller (the
database's list, kanban and schema, the canvas's layers, the presentation's slides, nine
samples in beui-demo) needs:

- a gap that opens, on a spring, where an item from another list would land, with that
  list's preview drawn in it;
- a row carried off its list collapsing as its slot closes, and its slot opening again when
  it comes back;
- glides for only the rows a move shifts;
- a sorted (`fixed`) list that takes items only where it sorts them;
- Alt and an arrow to step a row, announced for a screen reader.

Each caller would rebuild these, about as long as `Reorder` itself for the kanban alone.

## Rejected

- **Removing `Reorder` for `use_drop_site` + `slots_in` at each caller:** short only for a
  list that never takes items from another, never sorts and never animates.
