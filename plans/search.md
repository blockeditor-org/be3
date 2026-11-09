# Search

Depends on: none for steps 1 and 2; step 3 on plans/search_library.md (more
types).

Full-text search over names and content: what is left of it. Built
(guides/blocks/host.md, "Search"): the index block type `SEARCH_INDEX_TYPE` and its
access (exactly its root's, checked by the server), indexing on seal into the
root's index with commits sealed into the outbox and merged by parts, the
`search-watch` query on the app protocol, the file tree's filter listing hits
in content, and quick open listing hits. `plans/deferred/daily_driver.md`
("Search") asks for the rest before daily use.

## Order of work

Each step is one PR.

1. **`changed-since` and catching up.** Commits from devices that never index
   (old builds, `be` without it, a viewer's, another device's that left the
   session to this one) are found by comparing each document's indexed commit
   with the block's `head`. The host never lists a whole tree, so add a server
   call `changed-since(root, revision)` answering, a page at a time, the
   summaries of blocks in the tree changed after a revision. The server's graph
   revision restarts at zero with the process, so the call needs a durable
   change counter per block (a column the server bumps on every create,
   publish, move and metadata change). The indexer runs it from the index's
   watermark (`Batch::watermark`) when it opens a root's index and after
   reconnecting, indexes what is stale, and moves the watermark with each
   batch, so a long catch-up resumes where it stopped; `Searched::complete`
   says it is still catching up. Imports (`Peer::queue_import`) are caught up
   this way too, since no session seals them.
   - **Leaving the tree.** A block moved out of the tree, deleted to the trash
     or collected gets its documents masked (`Batch::removed_blocks`) by the
     device that moved it (it had Manage); its terms leave the head when
     compaction next merges that segment. Catching up masks what other devices
     missed. Today a query only drops hits the mirror shows detached or no
     longer viewable.
   - Done: a server test of `changed-since` paging; a host-stack test where one
     device writes with indexing off (`Searching::off`) and another indexes
     those commits on opening the root's index, resuming after a disconnect
     mid-catch-up; moving a block out of the tree drops it from results.
2. **Locations.** Opening a hit opens the block at its document and highlights
   the match where the editor supports a location: `OpenableBlock` (be-host's
   palette), `EditorAction::OpenBlock` and the app protocol's open request
   carry the document key and match, and the file tree's hit rows pass it on.
   Done: a host test opens a hit from quick open and the text shows the match
   selected, with a painting.
3. **More types**, as their `search-text` lands (plans/search_library.md).
   Once text answers `search-text`, the indexer asks the type for it (with the
   parts the index holds text for as `indexed`) instead of reading the plain
   file (`be_host_stack::search`'s `text_of`), keeps `source` per document, and
   cuts snippets from `search-text` with the hit's key; a type that loads whole
   is decoded whole for that, so keep the snippet text per (block, commit, key)
   as `cached_text` does.

## Left open

- **The writer only.** The device whose session seals a commit indexes it
  (`Status::owner`); a follower's edits are indexed by the owner device. A
  device that cannot edit the root never writes its index.
- **Compaction.** Each batch adds a segment; nothing merges them yet
  (`Index::compact` exists). Compact on the device that writes, a few segments
  at a time, when the count passes a threshold, and call
  `Index::forget_before` past the index block's history retention.
- **Edited outside a session.** The indexer seals commits straight into the
  outbox rather than through the index block's replica (`put-part` plus a
  `SetIndex` operation): two devices online at once diverge and merge by parts
  instead of being sequenced. `LiveEdit` has no hook for a replica to put a
  part of its own, which a session needs.
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
that it also sees index sizes, when they change, which parts change, and which
parts and ranges a query reads. It never sees a term, a query or a name.
