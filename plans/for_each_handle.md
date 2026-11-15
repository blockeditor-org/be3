# ForEach: each row's index, and a handle on what it built

Depends on: none.

`ForEach` (beui-tree `control.rs`) keeps a built row per key and drops it when the key goes. Code
around it often needs what it already knows: where a key sits now, and which nodes a key built.
Today `Reorder` keeps its own list of each key's glide nodes (`glides` in `reorder.rs`). Nothing
about animation or dragging goes into `ForEach`.

The user's queued control-flow spec, not yet a plan, also changes `ForEach`: every row takes
several children (a nested run, moved whole on a reorder, with a fast path for a row that is
exactly one fixed child), the list takes a fallback for when it is empty, and each row is handed
its index and a signal of its item (or an `Index` component lists by position). Its
`KeepAlive`/`Kept` replaces the keep-alive maps kept by hand (the dock's `panels`). This plan
takes the spec's row arguments as they are and adds only the handle. When the spec becomes a
plan, step 1 here moves into it and this plan depends on it.

## Shape

- **Row arguments**, as the spec has them: the row closure may take the key's index,
  `ReadSignal<usize>`, which changes when the order does (`<Glide on={index}>` glides a row when
  it moves, plans/glide_on_a_signal.md; striped rows; "3 of 10"), and a signal of its item.
- **Handle:** `ForEachRef<K>`, made like a `NodeRef` and passed as `handle`:
  - `nodes_of(&K) -> Vec<NodeId>`, the row's run (one node on the fast path), and `key_of(NodeId)`;
  - `index_of(&K)` and `keys()`, the order shown;
  - the node the `ForEach` fills.

  `VirtualList` gives the same handle; `nodes_of` is empty for a row it has not built.

## Steps

1. The row arguments on `ForEach` and `VirtualList`.
2. `ForEachRef` on both, on both cores.
3. Move `Reorder`'s `glides` onto the handle.
4. Tests: indices and `nodes_of` follow a key across a reorder, an insert and a removal
   (`nodes_of` is empty after it); a row of several children reports all of them.
