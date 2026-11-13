# ForEach: each child's index, and a handle on what it built

Depends on: none.

`ForEach` (beui-tree `control.rs`) keeps a built child per key and drops it when the key goes.
Code around it often needs what it already knows: where a key sits now, and which node a key
built. Today callers keep their own map from key to node (`Reorder`'s glides, the dock's bars).

## Shape

- **Index:** the row closure may take a second argument, `ReadSignal<usize>`, the key's current
  index. It changes when the order does: `<Glide on={index}>` glides a row when it moves
  (plans/glide_in_unstyled.md), and it serves striped rows and "3 of 10" labels.
- **Handle:** `ForEachRef<K>`, made like a `NodeRef` and passed as `handle`:
  `node_of(&K) -> Option<NodeId>`, `key_of(NodeId) -> Option<K>`, `index_of(&K)`, `keys()` (the order
  shown), and the node the `ForEach` fills. `VirtualList` gives the same handle; `node_of` is
  `None` for a row it has not built.

Nothing about animation or dragging goes into `ForEach`.

## Steps

1. The index argument and `ForEachRef` on `ForEach` and `VirtualList`, on both cores.
2. Move callers that keep their own key-to-node map onto the handle.
3. Tests: indices follow a reorder, an insert and a removal; `node_of` follows a key across a
   reorder and is `None` after removal.
