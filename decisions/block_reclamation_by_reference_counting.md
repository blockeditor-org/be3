# Blocks, commits and objects are reclaimed by reference counting

Every block has exactly one parent, and a block is live when its parent chain
reaches the root with every parent still listing its child
(`be-block-graph`). An account's Unfiled, which has no content, is the one
parent that holds its children by their parent alone. Storage is reclaimed by reference counting: a block holds
its commits, a commit holds its chunk hashes, and `ObjectRefs` deletes an
object when its last holder releases it.

## Why

With one parent per block, liveness is a walk up one chain, so there is no
cycle to trace. Reference counting then reclaims everything without a
collector that has to see the whole graph.

## Rejected

- Tracing or mark and sweep over the graph: unnecessary with single-parent
  liveness, and it would have to walk everything reachable.
