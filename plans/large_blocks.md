# Large blocks

A block loads, saves and publishes whole, so every save costs more as the block
grows, whatever changed. This plan makes a save cost what changed, and lets a
reader load part of a block. Chat needs it (plans/chat.md); the canvas, and any
block with many children or attachments, gains too. Saving less often is
plans/live_edit_persistence.md. Parts, the manifest tree, publishing changes
and per-part references (formerly 1–4) are done; 1a's wiring and 5 are left.

## What exists

- **Parts.** `BlockContent::parts` returns the stored form as a tree of
  `Part`s (new bytes, `Reused(PartRef)` from the loaded head, or a `Node` of
  children) whose leaves concatenate to `encode()`; `from_stored(&Stored)`
  gets the `LoadedPart` tree back. Each leaf is sealed on its own (split by the
  vault's chunker past its minimum), and `Blob` saves its header and payload as
  two parts (guides/blocks/content_types.md, "Parts").
- **The manifest tree.** `Manifest { content_type, root: Option<PartRef> }`
  names a tree of sealed `Node`s in the shape of the parts; unchanged subtrees
  are shared between commits, and a manifest may mix data key epochs. A
  be-block-client test (`a_large_block_saved_after_a_small_change_uploads_only_what_changed`)
  shows an 8 KiB edit to a 4 MiB block saving one chunk and two nodes.
- **Replicas.** A type module's replica loads a commit with its parts (the
  `load` host call answers a `Stored`) and saves parts, `reused` ones included
  (`written.parts` on the type protocol).
- **Loading part of a block.** A type whose `BlockContent::LOADS` is
  `Loads::FirstPart` loads only the root's first part; its replica, its
  followers and its editors hold only that, an editor reads the rest by range
  (`LiveContent::read_range`, answered through the replica and the `read-part`
  host call), and a save reuses what it never loaded
  (guides/blocks/apps.md, "Reading a range";
  `a_replica_that_loads_only_the_header_reads_the_payload_on_demand`). `Blob`
  supports it per kind (`BlobKind::LOADS`), but no kind uses it yet.
- **Range reads on the peer.** `Peer::stream_header`, `stream_range`,
  `read_span` and `read_part` fetch only the nodes and chunks a range touches.
- **Publishing** sends what changed since the expected head: an
  `ObjectChange` and an order-keeping `ReferenceChange`
  (`crates/be-block-protocol`); the server rebuilds the commit's object list
  from the expected head's rows and writes only the edges that changed
  (guides/blocks/server.md, "Publishing"). The server still stores a full
  `commit_objects` list per commit, written from the change.
- **References** are `BlockContent::references_in` plus each manifest node's
  own (`Part::Node { references }`), so a reused part keeps its references
  without being loaded (guides/blocks/content_types.md, "Parts").
  `Peer::references_of` lists a commit's.
- **Sessions.** A joining follower loads the head from the store itself, so
  for a type that loads its first part it gets no more than that.
- **Documents** save as parts chosen by their structure: list objects by id,
  large objects as parts of their own, text and other bytes by content-defined
  chunking (guides/blocks/content_types.md, "The stored form"). Typing in a
  1 MiB text block uploads one part and the nodes above it.

## The design

### 1a. Loading part of a document

Documents save as parts (guides/blocks/content_types.md, "The stored form")
but still load whole. Left:

- Each document part lists the references its own bytes hold
  (`Part::Node { references }`), but `references_in` still lists them all,
  because `describe` (the graph mirror) reads only `references_in`. A document
  that loads only some parts (5) must drop them from `references_in` and have
  `describe` add its parts' references.
- Loading part of a document is 5; the model could later offer it per field
  (an attribute such as `#[model(lazy)]` on a `List`), with a part's objects
  loaded when first read. Chat's segments don't need that, since they are its
  own layout.

### 5. What is left of loading part of a block

- **Blob kinds load only their header.** Image, audio, PDF, game module and
  workspace file editors read `Blob::data()` (`be-app-image/src/root/picture.rs`,
  `be-app-audio/src/root/ui.rs`, `be-app-pdf/src/root/pages.rs`,
  `be-app-deterministic-game`, the canvas's `export.rs`). Give
  `ContentProjection<Blob<K>>` a payload signal that holds `data()` when it is
  loaded and otherwise reads `[header length, stored_length)` with
  `read_range` once per snapshot, move those editors onto it, then set each
  kind's `BlobKind::LOADS` to `Loads::FirstPart`.
- **An operation that adds a part** (closing a chat segment) carries the
  part's hash. Every replica encodes the same bytes and checks the hash. Chat
  is its first user.

## Order of work

Chat needs what is left of 1a and 5; blobs need 5's first item.

## Tests

- An image editor shows an image whose replica loaded only its header.
