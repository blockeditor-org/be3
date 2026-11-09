# Search

Depends on: plans/search_library.md; plans/large_blocks.md steps 2 to 4
(`merge3` over parts, per-type retention, lazy rekeying); plans/command_palette.md for step 4 only (quick
open, which search fills with results).

Full-text search over names and content. The server never reads content
(guides/the_new_block_stack.md, goal 5), so clients build the index, encrypt
it like any block, and search it locally. `plans/deferred/daily_driver.md`
("Search") asks for it before daily use.

## The shape

- **One shared index per root block.** Access is the highest grant on the path
  up (decisions/block_access_is_the_highest_grant_on_the_path_up.md), so
  anyone who can view a root block (a workspace, My files) can view everything
  in its tree, and nested scope keys are wrapped under the root's. One index
  block per root, readable by exactly those people, holds the whole tree.
  Everyone with Edit on the root writes to it; viewers only read it.
- **Blocks shared without their root** (Shared with me) and what a viewer's
  root index lacks are not content-searchable until the personal index
  (`plans/deferred/search_personal_index.md`).
- **Every index is a block**, so index writes are saved offline-first like any
  other edit, and a device searches its own changes as soon as it sealed them.
- **Names are indexed too**, as a field of each document. There is no search
  over the graph mirror: the host keeps lazy-loading the tree and never lists
  a whole organization (guides/blocks/host.md, "The graph mirror"), so a block
  is found once the index holds it.
- **Scope**: only the open organization, never the trash.

A query fans out over the indexes of every root the account can view in the
open organization, merges by block (where two indexes hold different commits of a block, a
descendant wins, otherwise the later commit time), and drops
any block the mirror says the account can no longer view (an index can lag a
move or a revocation).

## The index block

- A block type (`SEARCH_INDEX_TYPE`) in a type module like any other
  (guides/type_modules.md), referenced from the root's defined metadata by a
  `SEARCH_INDEX` key, so it is held by its root and never listed among its
  children (guides/blocks/host.md, "Metadata").
- **Access only from its parent**, like settings blocks
  (decisions/settings_blocks_take_their_access_from_their_parent.md), but at
  the parent's level rather than capped: Edit on the root gives Edit on its
  index, View gives View. Its own grants and authorship never raise that.
  Never `ANYONE_INSIDE`: a block shared alone must not read the whole tree's
  text through it. `level_below` in `be-block-graph`.
- **The server enforces it by type and parent**, since it cannot read defined
  metadata: `SEARCH_INDEX_TYPE` is a constant in `be-block-graph` beside
  `SETTINGS_TYPE`, a block of that type under a root takes its access from the
  root as above, the server refuses a second one under the same root, and
  creating, moving or dropping it needs Edit on the root (checked like
  `require_keeping_settings`). The `SEARCH_INDEX` key is only how clients
  find it.
- **Never indexed itself**: the indexer skips blocks of `SEARCH_INDEX_TYPE`
  and the other metadata-held blocks (settings, edit kinds).
- **A large block** (guides/blocks/content_types.md, "Parts"): `LOADS` is
  `Loads::FirstPart`, and the first part is a small header (the segment list,
  each segment's top-level dictionary, the watermark: the graph revision it
  has seen every change up to). Under it, a `Node` per segment with its
  dictionary blocks, postings and positions as leaves, and the document table
  as a part split by document range. A query loads the header and reads the
  rest by range (`read-part`); a batch writes its new segment, the changed
  ranges of the document table and the header, and hands every other part
  back as `reused`, so nobody ever holds the whole index.
- **Hand-written content, not a `Document`**, as `Blob` is: documents load
  whole.
- **Edited through its replica's session.** The indexer opens the index block
  like any open block (`be_block_type_host::Replicas`) and sends a batch as an
  operation adding the segment's parts by hash (`put-part`, guides/type_modules.md, "Parts named by hash") with the
  document table entries. Two indexers online at once are sequenced by the
  session owner; offline batches merge with `merge3` over parts (large_blocks
  step 2), as `be-lib-search` merges two indexes.
- **History**: a short per-type retention policy (large_blocks step 3). Old
  index commits would keep terms of blocks that left the tree.
- Charged to the tree's owner like everything in the tree
  (decisions/storage_is_charged_to_the_trees_owner.md).

## The indexer

A task in the host-stack worker (`be-host-stack/src/worker.rs`), so `be` can
run it too. It never runs on a timer of its own (guides/blocks/host.md, "The
worker").

- **The writer indexes.** A device that seals a commit holds its plaintext
  already, so after each seal (the session owner's, online or offline, an
  import) it queues the block for its root's index when it can edit the root.
  It asks the block's type for `search-text`, passing the parts the index
  already holds text for as `indexed`, and sends one segment per batch.
- **Catching up.** Commits from devices that never index (old builds, `be`
  without it, a viewer's) are found by comparing each document's indexed
  commit with the block's `head`. The host never lists a whole tree, so add a
  server call `changed-since(root, revision)` answering, a page at a time, the
  summaries of blocks in the tree changed after a revision. The indexer runs
  it from the index's watermark when it opens a root's index and after
  reconnecting, indexes what is stale, and moves the watermark with each
  batch, so a long catch-up resumes where it stopped. Search answers from what
  is indexed so far and says the index is incomplete.
- **Leaving the tree.** A block moved out of the tree, deleted to the trash or
  collected gets its documents masked by the device that moved it (it had
  Manage); its terms leave the head when compaction next merges that segment.
  Catching up masks what other devices missed.
- **Rotation.** When a member loses access to the root, the index block rotates
  with the root's subtree (`Peer::rotate`); metadata-held blocks such as
  settings must be covered, and step 1's test checks it. Keys update lazily
  (large_blocks step 4), so a rotation costs the index nothing until parts are
  rewritten.

## Querying: the app protocol

- A search query of its own in `be-sdk` (`src/graph.rs`) and `be-sdk-api`, not
  a `BlockQuery` variant (that enum is `Copy`, and its answers are block ids):
  `Search { text, within }` answering hits of block, document key, snippet and
  match ranges. Plain data, nothing beui-specific. `within` limits it to a
  block's subtree.
- **Queries run in the host**, which links `be-lib-search` and reads the
  index's parts natively; the index's type module only writes and merges it,
  so there is no query request in the type protocol.
- The host reads indexes on demand, never in full: each segment's term
  dictionary, then ranges of postings with `read-part`. What it read stays in
  the object store (`be-objects/`, the origin private file system on the web),
  so later and offline queries reuse it.
- **Snippets** come from the content of the shown hits only: the host asks the
  hit's type for that document's text (`search-text` with its key) and cuts
  the snippet around the matched terms.
- Results are pushed again when an index changes, like any query.

## UI

- The file tree's filter field (today it matches loaded rows by name only)
  answers from the index once it exists.
- Quick open (`plans/command_palette.md`) lists search hits.
- Opening a hit opens the block at its document and highlights the match where
  the editor supports a location.

## Order of work

Each step is one PR.

1. **The index block type and its access.** `SEARCH_INDEX_TYPE`, the
   `SEARCH_INDEX` key, `level_below` and the server checks. Done: server tests
   that Edit on a root gives Edit on its index and View gives View, that a
   grant or authorship on the index raises nothing, that someone shared one
   block inside cannot open it, that a second `SEARCH_INDEX` and a reference
   made without Edit are refused, and that removing a member rotates the
   index block with the root's subtree (new epoch, the removed account's
   wraps gone).
2. **Indexing on seal, for text.** The indexer, the index's batch operation and
   merge, the search query and the filter field. Done: a host-stack test types
   into two text blocks of a workspace, finds each by a word, edits one and no
   longer finds the old word, finds an edit made offline once it is sealed,
   and two devices indexing at once both find every word after they sync; a
   painting of the filter showing hits with snippets.
3. **`changed-since` and catching up.** Done: a server test of
   `changed-since` paging; a host-stack test where one device writes with
   indexing off and another indexes those commits on opening the root's
   index, resuming after a disconnect mid-catch-up; moving a block out of the
   tree drops it from results.
4. **Quick open and locations.** Done: a host test opens quick open, types a
   word, sees the hit and opens the block at it, with a painting.
5. **More types**, as their `search-text` lands (plans/search_library.md).

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
