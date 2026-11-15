# ForEach: each row's index, and a handle on what it built

Depends on: none.

`ForEach` (beui-tree `control.rs`) keeps a built row per key and drops it when the key goes. Code
around it often needs what it already knows: where a key sits now, and which nodes a key built.
Today `Reorder` keeps its own list of each key's glide nodes (`glides` in `reorder.rs`). Nothing
about animation or dragging goes into `ForEach`.

`ForEach` already hands a row its item (by value, or as a signal with a `key`) and its
`index: ReadSignal<usize>`, builds a row of several children as a run moved whole on a
reorder, and takes a `fallback` for an empty list. This plan adds only the handle.

## Shape

- **Handle:** `ForEachRef<K>`, made like a `NodeRef` and passed as `handle`:
  - `nodes_of(&K) -> Vec<NodeId>`, the row's run (one node on the fast path), and `key_of(NodeId)`;
  - `index_of(&K)` and `keys()`, the order shown;
  - the node the `ForEach` fills.

  `VirtualList` gives the same handle; `nodes_of` is empty for a row it has not built.

## Steps

1. The row arguments on `VirtualList`, as `ForEach` has them.
2. `ForEachRef` on both, on both cores.
3. Move `Reorder`'s `glides` onto the handle.
4. Tests: indices and `nodes_of` follow a key across a reorder, an insert and a removal
   (`nodes_of` is empty after it).
