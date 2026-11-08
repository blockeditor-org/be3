# Search

A plan for full-text search over names and content. The server never reads
content (guides/the_new_block_stack.md, goal 5), so clients build the index,
encrypt it like any block, and search it locally. `plans/daily_driver.md`
("Search") asks for it before daily use.

## The shape

- **One shared index per root block.** Access is the highest grant on the path
  up (decisions/block_access_is_the_highest_grant_on_the_path_up.md), so
  anyone who can view a root block (a workspace, My files) can view everything
  in its tree, and nested scope keys are wrapped under the root's. One index
  block per root, readable by exactly those people, can hold the whole tree.
  Everyone with Edit on the root writes to it; viewers only read it.
- **One personal index per account** for blocks it can view without viewing
  their root: the Shared with me entry points (a viewable block whose parent is
  not viewable) and their subtrees. It lives with the account's own `Settings`,
  so only that account reads it, and is partitioned by entry point.
- **No local-only layer.** Every index is a block, so index writes are saved
  offline-first like any other edit (a local commit in the outbox, published
  on the next drain), and a device searches its own changes as soon as it
  sealed them. A viewer of a root indexes what the root's index lacks into its
  personal index. An account with no personal space to save to searches only
  what the indexes it can read already hold.
- **Names come from the graph mirror** as well as the index, so a block whose
  content no index holds yet is still found by name.

A query fans out over the indexes of every root the account can view in the
open organization and the personal index, merges by block (the newer indexed
commit wins), and
drops any block the mirror says the account can no longer view (an index can
lag a move or a revocation).

## The index format: `be-lib-search`

A pure crate, native and wasm, no IO:

- **Tokenizing**: `unicode-segmentation` word boundaries and `caseless`
  folding (both already in `Cargo.lock`; `plans/i18n.md` wants case-folded
  matching too). Prefix matching on the last term; fuzzy matching only on
  names.
- **Documents** are `(block, part)` pairs, not blocks. The part is a leaf of
  the block's own stored parts (guides/blocks/content_types.md, "Parts"), so a
  large block (a chat's segments, a long text's sections) needs no sub-blocks:
  a commit that reuses a part leaves its document alone, a changed part is
  reindexed by reading that part alone, and a hit names where in the block it
  is. A small block is one document. Fields: name, body.
- **Segments**: an immutable batch of documents, saved as separate parts so a
  query reads only what it needs, each a few range reads at most:
  - a term dictionary in sorted blocks of a few KiB, with a small top level of
    each block's first term (an `fst` over those). A lookup reads the top
    level and one block; a whole `fst` over every term would be tens of MiB in
    a large segment and its nodes scatter across the part;
  - document postings (ids and frequencies) in blocks of 128 with skip data
    and each block's highest score, so a query stops reading a common term's
    list once no remaining document can enter the top results (block-max
    WAND);
  - positions apart from postings, read only for phrase queries and snippets;
  - the extracted text, compressed per document, read for the hits shown,
    so snippets need no reads of the blocks themselves.
- **The document table**: per document, the head commit it was indexed at and
  the segment holding it, sorted by document. A newer entry masks older
  postings; each segment also keeps a deletion bitmap so masking never reads
  the table. Content-defined chunking (guides/blocks/server.md) means a batch
  touching a few documents re-uploads a few chunks of it, and a writer reads
  only the ranges for the documents it reindexes.
- **Prefixes**: the last term matches as a prefix only from two characters,
  and a prefix expands to at most a bounded number of terms (the most frequent
  ones), so a short prefix never reads a large share of the dictionary.
- **Ranking**: BM25 over body, a strong boost for name hits, recency from the
  summary as a tie-break.
- **Compaction** is tiered: segments of similar size merge, so the count stays
  logarithmic in the index and a document is rewritten a logarithmic number of
  times. It runs only on devices that are on power and a fast connection, and
  a merge of large segments is rare and resumable (each output segment is its
  own commit).

Segments, not one index rewritten in place, because a batch then adds parts
without touching the others: a save reuses every existing segment, and two
devices' batches merge as a union.

Rejected for now: tantivy. It wants a filesystem directory, threads and its own
merge policy, and its segments cannot be three-way merged by our commit DAG.

## The index block

- A block type (`SEARCH_INDEX_TYPE`) in a type module like any other
  (guides/type_modules.md), referenced from the root's
  defined metadata by a `SEARCH_INDEX` key, so it is held by its root and never
  listed among its children (guides/blocks/host.md, "Metadata").
- **Access only from its parent**, like settings blocks
  (decisions/settings_blocks_take_their_access_from_their_parent.md), but at
  the parent's level rather than capped: Edit on the root gives Edit on its
  index, View gives View. Never `ANYONE_INSIDE`: a block shared alone must not
  read the whole tree's text through it. `level_below` in `be-block-graph`, and
  a server check that creating or dropping the reference needs Edit on the root.
- **A large block** (guides/blocks/content_types.md, "Parts"): `LOADS` is
  `Loads::FirstPart`, and the first part is a small header (the segment list,
  each segment's top-level dictionary, the watermark: the graph revision it
  has seen every change up to). Under it, a `Node` per segment with its
  dictionary blocks, postings, positions and text as leaves, and the document
  table as a part split by document range. A query loads the header and reads
  the rest by range (`read-part`); a batch writes its new segment, the changed
  ranges of the document table and the header, and hands every other part back
  as `Reused`, so nobody ever holds the whole index.
- **Hand-written content, not a `Document`.** The module protocol already
  carries what a large type needs: `loaded.parts` names every saved part,
  `written-part::reused` keeps one without its bytes, and `read-part` reads a
  range. Documents (`be_block_model`) save as parts but load whole, so the
  index implements the content traits by hand, as `Blob` does.
- **Edited through its replica's session.** The indexer opens the index block
  like any open block (`be_block_type_host::Replicas`) and sends a batch as an
  operation: it uploads the segment's parts first, and the operation carries
  their hashes and the document table entries, the operation that adds a part
  by its hash that `plans/chat.md` already needs. The operation is small, so it
  journals and relays like any edit, and two indexers online at once are
  sequenced by the session owner rather than merged. `apply` is not used: it
  takes and returns whole bytes.
- **Merging over parts.** Offline batches merge without reading any segment:
  segments are a union by id, the document table keeps the newest commit per
  document (reading only the ranges both sides changed), the watermark the
  lower of the two, and nothing conflicts. This needs `merge3` over parts
  (`plans/large_blocks.md`). Two devices indexing the same commit at
  once cost a duplicate segment, which compaction drops.
- **History**: a short retention policy for this content type, so pruning
  drops old states soon. Old index commits would keep text of blocks that left
  the tree readable to the tree's members. Retention (`RetentionPolicy`) is
  per organization today; a per-type one is in `plans/large_blocks.md`.
- Charged to the tree's owner like everything in the tree
  (decisions/storage_is_charged_to_the_trees_owner.md); its size is about the
  tree's text.
- Two devices that each make one before seeing the other's: read both, write
  through the lowest id, as `Settings::merged` does; or have the server refuse
  a second `SEARCH_INDEX`.

## Extracting text: a type-module request

A stateless `search-text` request beside `describe` and `to-file`
(guides/type_modules.md, "The protocol"; WIT in `be-sdk-type-api`), backed by a
`BlockContent` method with an empty default: a type that does not answer is
found by name only. It is asked for the parts of a commit that changed since
the indexed one (compared by `PartRef`), answering a document (name, body
text) per part, so a large block is read only where it changed; a type that
loads only its first part (`Loads::FirstPart`) reads the others through
`read-part`. First: text and markdown, checklist, database rows, canvas
text items, the tracker. Later: chat (by segment), PDF text, OCR never on the
server.

## Writing the index: the indexer

A task in the host-stack worker (`be-host-stack/src/worker.rs`), so `be` can
run it too. It never runs on a timer of its own (guides/blocks/host.md, "The
worker").

- **The writer indexes.** A device that seals a commit holds its plaintext
  already, so after each seal (the session owner's, online or offline, an
  import) it queues the block for its root's index, or for the personal index
  when it cannot edit the root. Each batch is one segment, sent as an
  operation on the index's replica; offline the replica owns a made-up session
  and seals locally (guides/blocks/host.md, "Opening offline"), so offline
  edits are searchable at once and reach other devices with the content they
  index.
- **Catching up.** Commits from devices that never index (old builds, `be`
  without it, a viewer's) are found by comparing each document's indexed
  commit with the block's `head` in its summary. The host never lists a whole
  tree, so add a server call `changed-since(root, revision)` answering, a page
  at a time, the summaries of blocks in the tree changed after a revision (the
  server knows the graph and revisions already). The indexer runs it from the
  index's watermark when it opens a root's index and after reconnecting,
  indexes what is stale, and moves the watermark with each batch it publishes,
  so a long catch-up resumes where it stopped.
- **The first index of a large tree** means reading every block's content once
  (`read_contents`, many heads per round trip) and is the slow case: for a
  gigabyte of text, a download of the whole tree and a few minutes of
  tokenizing. Several devices share it: each claims a range of the
  `changed-since` pages by publishing an empty batch naming it, and a device
  skips ranges another already claimed. Search answers from what is indexed
  so far and says the index is incomplete.
- **Leaving the tree.** A block moved out of the tree, deleted to the trash or
  collected gets its documents masked by the device that moved it (it had
  Manage); its text leaves the head when compaction next merges that segment,
  since everyone who can read the index could read the block until it moved.
  Catching up masks what other devices missed.
- **Rotation.** The index block must rotate with its root's subtree when a
  member loses access (`Peer::rotate`); check that metadata-held blocks such
  as settings are covered. Keys update lazily (`plans/large_blocks.md`), so
  a rotation costs the index nothing until parts are rewritten.
- **Viewers** read the shared index and write what it lacks (stale or
  missing documents they find while catching up) into their personal index.

## The personal index

- A system block of the account under its `Settings`, so only it views it.
  Partitioned by entry point: each partition has its own segments and document
  table.
- An entry point inside another of the account's entry points indexes into the
  outer one's partition.
- A partition per root the account may only view holds what that root's index
  lacks; an entry the root's index has a newer commit for is masked at query
  time and dropped by compaction.
- A partition is dropped when its entry point leaves the `Shared` listing, when
  the account gains view of its root (the root's index takes over), or, for a
  viewed root, when the account loses view of it or gains Edit.
- Charged to the account, as its own tree.

Alternative set aside: a shared index under each entry point for all its
grantees. Members of the root would index those blocks twice, and the index
would be paid by the tree's owner for people the owner may not know about.

## Querying: the app protocol

- `BlockQuery::Search { text, within }` in `be-sdk` (`src/graph.rs`) and
  `be-sdk-api`, answering block ids with the part, a snippet and match ranges.
  Plain data, nothing beui-specific (the app protocol stays
  framework-independent). `within` limits it to a block's subtree.
- The host reads indexes on demand, never in full: each segment's term
  dictionary, then ranges of postings with `read-part`'s range reads
  (guides/type_modules.md, "Reading a range"), then the stored text of the hits
  it shows. What it read stays in the object store (`be-objects/`, the origin
  private file system on the web), so later queries and offline queries reuse
  it.
- Results are pushed again when an index changes, like any
  query (`graph_revision`).

## UI

- The file tree's filter field (today it matches loaded rows by name only).
- A quick-open (Ctrl+P / Ctrl+K) in both shells, with the command palette
  (`plans/command_palette.md`).
- Opening a hit opens the block at its part and highlights the match where the
  editor supports a location.
- Paintings for the filter, quick-open and results.

## Scale

The target is a tree with a gigabyte of text (about 150 million words):

- The index is roughly 0.6 to 0.8 GB: postings about 10% of the text,
  positions 20 to 30%, stored text about 30% (zstd), dictionaries the rest.
  The tree's owner pays for it.
- A query reads per segment a top-level dictionary, one dictionary block per
  term and the postings blocks it does not skip: with about 20 segments after
  tiered compaction, a few hundred KiB to a few MiB and three or four round
  trips (segments in parallel), less once cached.
- A batch from an ordinary edit writes one small segment and a few chunks of
  the document table.
- What does not scale and is accepted: the first index of a large tree (see
  "Catching up"), and a large merge, both bounded to capable devices and
  resumable.

## Order of work

1. Names: `BlockQuery::Search` over the graph mirror's names, wired to the
   filter and quick-open. Useful alone, and fixes the query's shape.
2. `be-lib-search`: tokenizer, segments, queries, merging, compaction, with
   unit tests.
3. `search-text` in the type protocol, for text and markdown first.
4. What it needs from `plans/large_blocks.md`: `merge3` over parts, lazy
   rekeying, per-type retention and the operation that adds a part by its
   hash.
5. The index block type, its access rule, the `SEARCH_INDEX` reference and the
   server checks.
6. The indexer: index on seal (offline too), `changed-since`, catching up,
   leaving the tree, rotation.
7. The personal index, with viewers' partitions.
8. More types, snippets in editors, locations.

## What the server learns

The aim is the same as for every block: hide names and content. The server
already knows the graph and which blocks each device opens, so it is accepted
that it also sees index sizes, when they change, which parts change, and which
parts and ranges a query reads. It never sees a term, a query or a name.

## Open questions

- Should search reach other organizations the account belongs to, or only the
  open one?
- Should the trash be searchable (it is detached, so outside any root's
  index)?
- Should the index store extracted text for snippets, or fetch the top hits'
  content instead? Storing it roughly doubles the text a tree keeps.
