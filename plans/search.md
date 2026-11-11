# Search

Depends on: none for steps 1 and 2; step 3 on plans/search_library.md (more
types answering `search-text`).

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
   call `changed-since(root, sequence)`. Designed in "Step 1, designed" below.
   The indexer runs it from the index's watermark (`Batch::watermark`) when it
   opens a root's index and after reconnecting, indexes what is stale, and moves
   the watermark with each batch, so a long catch-up resumes where it stopped;
   `Searched::complete` says it is still catching up. Imports
   (`Peer::queue_import`) are caught up this way too, since no session seals
   them.
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
3. **More types**, as they answer `search-text` (plans/search_library.md);
   the indexer already puts one document per answered key. With them, keep
   each document's `source` and pass the parts the index holds text for as
   `indexed`, so reindexing a large block reads only its changed parts.

## Step 1, designed

**What exists.** `blocks.version` is a per-block counter bumped on publish,
metadata change, move and the like (`blocks.rs`: `version = version + 1`). It
orders one block's states, not changes across blocks, so it cannot answer "what
changed after X". The graph's `revision` (`ServerStore::with_graph`) is
org-wide and ordered, but an in-memory counter that restarts with the process.

**The counter.** An organization-wide, durable, strictly increasing sequence:

- `organizations.change_sequence INTEGER NOT NULL DEFAULT 0`, and
  `blocks.changed INTEGER NOT NULL DEFAULT 0` with
  `CREATE INDEX blocks_changed ON blocks (organization_id, changed)`.
- Every write that bumps `version` also sets `changed = next sequence` in the same
  transaction (`next` is `UPDATE organizations SET change_sequence =
  change_sequence + 1 ... RETURNING change_sequence`, under the lock
  `with_graph` already holds for the organization): create, publish, move,
  metadata and references, trash, and a rotation that reseals the block.
- Hard removal (collection) leaves a row in `block_removals (organization_id,
  block_id, changed)` so it can be reported once; rows older than the retention
  of the index's history (a day, `be_server::retention`) are dropped. No
  migrations: the schema changes as it is (guides/frozen_formats.md).

**The call.** `blocks.changed-since(root, sequence, limit)` answers
`{ summaries: Vec<BlockSummary>, removed: Vec<Uuid>, next: Option<Cursor>,
sequence: u64 }`, a page of at most `limit` blocks, ordered by `(changed, id)`:

- Which blocks: those with `changed > sequence` that the caller may view and
  that are in the tree of `root` NOW (`BlockGraph::is_within`, checked in the
  cached graph; the scan is the index range, never the tree) , plus `removed`:
  blocks with `changed > sequence` that are NOT in the tree now (moved out,
  trashed, collected) and that the caller may view or could have viewed. The
  server cannot know which of them the tree held before, so the client intersects
  `removed` with the blocks its index holds documents for (`entries_of`) and masks
  only those. `removed` carries ids only, nothing a viewer of the root could not
  already learn from the graph.
- `sequence` is the organization's current sequence when the page was made; with
  `next` (a cursor of the last `(changed, id)`), the client pages until `next` is
  None and then stores `sequence` as the new watermark. A page never skips a
  change: pages are ordered and a write during paging gets a larger sequence, so
  the next catch-up sees it.
- Access: every call states its requirement in `rule`: signed in with the
  organization open, and the caller must view `root` (a viewer reads, so it can
  catch up locally as it searches; only an editor writes the index).

**The client.** `Peer::changed_since(root, watermark)` pages the call;
`be_host_stack::search` runs it when a root's index opens and after a
reconnect, for each stale block (head differs from the document's indexed
commit) queues `Searching::dirty` as a sealed block is queued, masks the
`removed` it holds with `Batch::removed_blocks` at the commit the mirror shows
(or the removal's `changed`), and writes the watermark with each batch. A
device that moves, trashes or collects a block masks it at once (it knows the
tree), without waiting for a catch-up.

**Decided here, change if wrong.**

- One org-wide sequence, not one per tree: a tree query filters in the cached
  graph. A per-root sequence would need every move to update two trees' counters.
- The scan is by `blocks_changed`, so a catch-up costs the changes since the
  watermark, not the tree, which is what the million-block target (below) needs.
- Removals are kept for a day only. An index that was last written longer ago than
  that catches up the removed documents on its next compaction of a segment that
  holds them, or never (a stale document hidden by `show`): stated in "Left open".

**Open questions for the user.**

- Is the one-day removal log acceptable, or must a device that returns after a
  week also mask what left the tree (a longer log, or a per-root rebuild)?
- May a viewer's device run the catch-up and cache results locally only, never
  write the index (it cannot), so its searches see stale text until an editor
  catches up? Default: yes, a viewer only reads.

## Left open

- **Leaving the tree is not masked** (until step 1) (step 3 of "The indexer"): a block moved to
  a private root, trashed or emptied keeps its text in the old root's index, which
  every viewer of that root can decrypt. The UI hides it (`show` filters by
  access) but a modified client reads it. Mask it with `Batch::removed_blocks`
  from the device that moves it.
- **Late text in a huge document is not searchable.** A document's text is
  indexed up to 1 MiB (`MAX_TEXT`); a word past that finds nothing, and nothing
  in the interface says why. It is capped after the body is loaded, so it does
  not bound memory, and the number of documents per block is not capped.
- **A trashed index is not restored.** From Detached it can only return to
  where it was, which the server refuses for an index; a new one is made.
- **A forged index.** Any editor of a root is enough to write arbitrary content into its
  index: forged entries, an unbounded size that every viewer's search reads, and a
  far-future commit time that wins the merge by commit time in a query. Cap an
  index's size and refuse commit times in the future when it is cheap.
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
that it also sees that a workspace has an index, its size, when it changes, which parts change, and which
parts and ranges a query reads. It never sees a term, a query or a name.
