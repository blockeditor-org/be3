# A block's live edits are sequenced by its session owner, not merged as a CRDT

The session owner sequences every operation on a block, so operations apply in
one order on every replica and nothing is rebased by position. Text and lists
share one ordered sequence (`be-lib-sequence`), whose positions are
`(client, offset)` like Yjs's `(client, clock)`, but whose order comes from the
owner. There is no CRDT.

