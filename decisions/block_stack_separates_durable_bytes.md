# A block's durable bytes are separate from live editing, undo, the graph and children

The block stack (`be-block-*`, guides/the_new_block_stack.md) keeps a block's
persistent form as a byte string in chunks with a commit chain over it, and
treats live editing, undo, the reference graph and child changes as separate
layers a content type opts into (`BlockContent`, `LiveEdit`, `Merge`, `Undo`,
`Streamed`). Live edits converge through a session owner; offline divergence
merges three ways over the persistent bytes; version control is built on the
store and commits rather than as block types.

## Why

The block system it replaced had one `Block` trait that fused the durable
representation, the live-edit operation, undo history, the reference graph and
child manipulation, and a `const CRDT: bool` that chose live merge, offline
merge and the wire protocol together. Most of what went wrong followed from
that coupling:

- The operation log never compacted. Reading a block replayed every operation
  it had ever received, and it could not be fixed in place: operations were
  ciphertext to the server, and compaction needs understanding. An append-only
  operation log and end-to-end encryption are incompatible.
- References cost an organization per keystroke. The client replayed pending
  operations to diff the full reference set, and the server rewrote every
  reference row in the organization on every update. Under a CRDT the delta
  was also wrong, because it was computed against an optimistic local state
  rather than the converged one. References are now recorded once per commit,
  from the sealed state.
- There was no local persistence. Unsent work lived in memory, so closing the
  process lost it.
- Bulk data was in band. An image was base64'd into JSON, wrapped in a
  snapshot, and shipped whole on every replace.
- The layering was inverted. Version control was a content-addressed blob
  store with commits and branches, built as block types on top of the block
  system, so the substrate was a guest.

Three-way merge over the persistent bytes makes a wholesale rewrite conflict
visibly instead of interleaving.

## Rejected

- One trait fusing content, live edit, undo, graph and children: the coupling above.
- A CRDT as the basis of every type, chosen by a flag: it tied live merge,
  offline merge and the wire protocol to one choice. A type may still be a
  CRDT where that is genuinely better, but nothing depends on it.
- An append-only operation log as the durable form: it cannot compact under
  end-to-end encryption.
- Version control as block types over the block system: the substrate would be a guest.
