# Search

Depends on: none.

Full-text search over names and content: what is left of it. Built
(guides/blocks/host.md, "Search"): the index block type `SEARCH_INDEX_TYPE` and its
access (exactly its root's, checked by the server), indexing on seal into the
root's index with commits sealed into the outbox and merged by parts, the
`search-watch` query on the app protocol, the file tree's filter listing hits
in content, quick open listing hits, and catching up through `changed-since`
(guides/blocks/server.md, "Changes since"; guides/blocks/host.md, "Catching
up").

## Order of work

1. **Locations.** Opening a hit opens the block at its document and highlights
   the match where the editor supports a location: `OpenableBlock` (be-host's
   palette), `EditorAction::OpenBlock` and the app protocol's open request
   carry the document key and match, and the file tree's hit rows pass it on.
   Done: a host test opens a hit from quick open and the text shows the match
   selected, with a painting.

More types answering `search-text` are in plans/search_library.md.

## Left open

- **A block let go stays stale.** A block whose commit could not be read three times is
  counted as indexed (`MAX_READ_FAILURES`), so a catch-up ends: its entry in the index is
  stale or missing until the block next changes. Answers are filtered by the mirror, so
  only snippets and ranking use the stale text.
- **A forged index.** Any editor of a root is enough to write forged entries into its
  index, or a commit whose own time (which the server cannot read) is far in the
  future. Closed: the server bounds a commit of an index (`MAX_INDEX_BYTES`), and a
  device does not believe a commit time past the later of its clock and the
  server's (as it measured it) plus `MAX_COMMIT_AHEAD`, whether it indexes it or
  reads it from an index (`Commit::believed`: it compares as the oldest, so the next
  honest put replaces it and it ranks last among equal scores, while the index
  stores every time as given, so a device whose clock is behind rewrites none). Left: the content itself, which only the root's editors can forge, and a
  forged time within the hour, which wins ties for that long. A merge of two index
  heads has no clock, so it keeps a forged time until the next apply.
- **The writer only.** The device whose session seals a commit indexes it
  (`Status::owner`); a follower's edits are indexed by the owner device. A
  device that cannot edit the root never writes its index.
- **Compaction.** Each batch adds a segment; nothing merges them yet
  (`Index::compact` exists), and `Index::forget_before` has no caller, so the
  index's header keeps every block it ever masked. Compact on the device that writes, a few segments
  at a time, when the count passes a threshold, and call
  `Index::forget_before` past the index block's history retention.
- **Rotation** reseals every part the index reuses at its next save, so a
  rotation costs one re-upload of the index;
  plans/deferred/large_blocks_lazy_rekeying.md would remove that.
- **Blocks shared without their root** (Shared with me) and what a viewer's
  root index lacks are not content-searchable until the personal index
  (`plans/deferred/search_personal_index.md`).

## Scale

Design notes, not done criteria. The target is a tree with a gigabyte of text
(about 150 million words):

- The index is roughly 0.3 to 0.5 GB: postings about 10% of the text,
  positions 20 to 30%, dictionaries the rest. The tree's owner pays for it.
- A query reads per segment a top-level dictionary, one dictionary block per
  term and the postings blocks it does not skip: with about 20 segments after
  tiered compaction, a few hundred KiB to a few MiB and three or four round
  trips (segments in parallel), less once cached.
- A batch from an ordinary edit writes one small segment and a few chunks of
  the document table.
- What does not scale and is accepted: the first index of a large tree, and a
  large compaction, both resumable. be-lib-search's compaction is neither
  yet: it reads its input segments whole into memory and writes the merged
  one in a single call.

## What the server learns

The aim is the same as for every block: hide names and content. The server
already knows the graph and which blocks each device opens, so it is accepted
that it also sees that a workspace has an index, its size, when it changes, which parts change, and which
parts and ranges a query reads. It never sees a term, a query or a name.
