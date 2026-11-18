# A block's live edits are ordered by the server, whose journal is the log

Every operation on a block in a live session goes to the server, sealed with the block's key.
The server numbers it (`be_block_session::Sequencer`), keeps it in the block's journal and sends
it to every participant, its author too; every replica applies that one order. Operations are
plain data whose positions are ids (`be-lib-sequence`'s `(client, offset)`, like Yjs's
`(client, clock)`), so they apply in any replica's state as numbered: nothing is rebased by
position, and there is no CRDT. The journal is the log: it is durable, a replica opening reads
it, and the lease holder's seal trims it. The server never opens what it numbers: it sees ids,
bases, floors and times, never content.

## Why

- **One trip.** An edit goes author, server, everyone. When one client (the session owner)
  ordered, an edit went author, server, owner, server, everyone, and the owner was often a
  phone elsewhere: two people at one desk waited on it, and on its link.
- **No handover.** Ordering moved with ownership, which needed numbering to be inherited,
  operations accepted late renumbered, catch-ups re-sent to each new owner and stashes handed
  over and acknowledged. The server is already on every path and always there, so the lease
  only decides who seals, and taking it moves no state.
- **The log is the journal.** The owner appended what it sequenced to a server journal anyway,
  so the server held the log already; numbering where it is kept removes the copy.
- **Determinism stays the replicas'.** Every replica of a session runs one build of the type
  (the session's tag, which the server holds every submission to), so applying the same order
  gives the same content without the server reading anything.

## What was rejected

- **Owner sequencing** (this decision's predecessor): the extra trip and the handover above.
- **A CRDT per type**: merging and undo get harder and every editor would carry its metadata;
  a block that is better as a CRDT can still be one (VISION.md, layer 4).
- **Rebasing by position**: needs an operation-transform function per type; ids make it
  unnecessary.

## Accepted limits

- The server can drop, hold back or replay an entry, or number it differently for different
  participants, which would leave their copies different. It cannot read or forge one. Fork
  detection would catch the last (guides/blocks/keys.md, "Accepted limits", "Forks").
- Offline there is no server: the device numbers in its own order over its own journal, and
  what it orders reaches the others as a commit that merges, not as operations.
