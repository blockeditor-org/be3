# Search: set aside

Depends on: none.

What is left of full-text search (built: guides/blocks/host.md, "Search"), set
aside until the user picks it up. The limits below are accepted for now.

- **Fuzzy matching on names**, which needs a way to find a misspelt name's
  terms without reading a segment's whole dictionary (the blocked dictionary
  answers exact terms and prefixes only). Set aside by the owner.
- **PDF text** answering `search-text`. Set aside by the owner. Not by linking
  pdfium into the pdf type module, which the owner ruled out (nor could it link
  as is: pdfium needs libc and more of WASI than the few calls a pure guest is answered). One way left open:
  the pdf app, which runs pdfium, saves each page's text as a part of the
  block, and the type module answers from that part alone; a pure-Rust parser
  in the type module reads far fewer real PDFs.
- **No host test for a second device.** be-host-tests start one host with its own
  dev server, so catching up what another device wrote, and a host restart, are
  only tested in be-host-stack, whose tests cover both. Set aside: be-host-tests
  would need a harness for a second device and a host restart.

## Limits accepted

- **Leaving the tree is masked on the next connection** (`changed-since`, built),
  not at once: a block moved to a private root, trashed or emptied keeps its text
  in the old root's index until a device that may edit the root next catches up,
  and only the catch-up masks it (no mask at the moment of the move). Until then
  every viewer of that root can decrypt it with a modified client; the UI hides it
  (`show` filters by access and by what the search is within). Departures are kept a
  day on the server: a device that was away longer misses every block that left in
  between, moved out, trashed or collected (it is only hidden by `show`).
- **A viewer's device does not catch up.** Only an editor of the root writes its
  index, so a viewer's searches show the text the last editor catch-up indexed.
  plans/deferred/search_personal_index.md would cover it.
- **Edited outside a session.** The indexer seals commits straight into the
  outbox rather than through the index block's replica (`put-part` plus a
  `SetIndex` operation): two devices online at once diverge and merge by parts
  instead of being sequenced. `LiveEdit` has no hook for a replica to put a
  part of its own, which a session needs.
- **Late text in a huge document is not searchable.** A document's text is
  indexed up to 1 MiB (`MAX_TEXT`); a word past that finds nothing, and nothing
  in the interface says why. It is capped after the body is loaded, so it does
  not bound memory, and the number of documents per block is not capped.
- **A read that fails outside a catch-up waits.** A block sealed here whose
  commit cannot be read is held back, not retried at once: it is read again
  when it next changes or a catch-up lists it. Only a block a catch-up let go
  (`MAX_READ_FAILURES`) is kept and read again by every later catch-up.
- **Edit access lost forgets what was let go.** A root this device can no
  longer edit drops its let-go blocks (`Peer::forget_let_go`): if access comes
  back, they stay stale until they next change.
- **A trashed index is not restored.** From Detached it can only return to
  where it was, which the server refuses for an index; a new one is made.
- **A forged index.** Any editor of a root is enough to write forged entries
  into its index, or a commit whose own time (which the server cannot read) is
  far in the future. Closed: the server bounds a commit of an index
  (`MAX_INDEX_BYTES`), and a device does not believe a commit time past the
  later of its clock and the server's (as it measured it) plus
  `MAX_COMMIT_AHEAD`, whether it indexes it or reads it from an index
  (`Commit::believed`: it compares as the oldest, so the next honest put
  replaces it and it ranks last among equal scores, while the index stores every
  time as given, so a device whose clock is behind rewrites none). Left: the
  content itself, which only the root's editors can forge, and a forged time
  within the hour, which wins ties for that long. A merge of two index heads has
  no clock, so it keeps a forged time until the next apply.
- **The writer only.** The device whose session seals a commit indexes it
  (the lease holder); edits another device made are indexed by the holder. A
  device that cannot edit the root never writes its index.
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
