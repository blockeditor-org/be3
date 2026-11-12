# Search

Depends on: none for step 1; step 2 on plans/search_library.md (more
types answering `search-text`).

Full-text search over names and content: what is left of it. Built
(guides/blocks/host.md, "Search"): the index block type `SEARCH_INDEX_TYPE` and its
access (exactly its root's, checked by the server), indexing on seal into the
root's index with commits sealed into the outbox and merged by parts, the
`search-watch` query on the app protocol, the file tree's filter listing hits
in content, quick open listing hits, and catching up through `changed-since`
(guides/blocks/server.md, "Changes since"; guides/blocks/host.md, "Catching
up").

## Order of work

Each step is one PR.

1. **Locations.** Opening a hit opens the block at its document and highlights
   the match where the editor supports a location: `OpenableBlock` (be-host's
   palette), `EditorAction::OpenBlock` and the app protocol's open request
   carry the document key and match, and the file tree's hit rows pass it on.
   Done: a host test opens a hit from quick open and the text shows the match
   selected, with a painting.
2. **More types**, as they answer `search-text` (plans/search_library.md);
   the indexer already puts one document per answered key. With them, keep
   each document's `source` and pass the parts the index holds text for as
   `indexed`, so reindexing a large block reads only its changed parts.

## Left open

- **Leaving the tree is masked on the next connection** (`changed-since`, built),
  not at once: a block moved to a private root, trashed or emptied keeps its text
  in the old root's index until a device that may edit the root next catches up,
  and only the catch-up masks it (no mask at the moment of the move). Until then
  every viewer of that root can decrypt it with a modified client; the UI hides it
  (`show` filters by access and by what the search is within). Departures are kept a
  day on the server: a device that was away longer misses every block that left in
  between, moved out, trashed or collected (it is only hidden by `show`).
- **A block let go stays stale.** A block whose commit could not be read three times is
  counted as indexed (`MAX_READ_FAILURES`), so a catch-up ends: its entry in the index is
  stale or missing until the block next changes. Answers are filtered by the mirror, so
  only snippets and ranking use the stale text.
- **No host test for a second device.** be-host-tests start one host with its own
  dev server, so catching up what another device wrote, and a host restart, are
  only tested in be-host-stack: a harness for a second device and a host restart.
- **A viewer's device does not catch up.** Only an editor of the root writes its
  index, so a viewer's searches show the text the last editor catch-up indexed.
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
