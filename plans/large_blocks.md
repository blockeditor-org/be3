# Large blocks

A block loads, saves and publishes whole, so every save costs more as the block
grows, whatever changed. This plan makes a save cost what changed, and lets a
reader load part of a block. Chat needs it (plans/chat.md); the canvas, and any
block with many children or attachments, gains too. Saving less often is
plans/live_edit_persistence.md. Parts and the manifest tree (formerly 1 and 2)
are done; the rest is not implemented yet.

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
- **Replicas.** A type module's replica sends its parts' shape with the bytes
  (`written.parts` on the type protocol), so its saves are parts too. It holds
  no `PartRef`s, so nothing it saves is `Reused`: unchanged parts are sealed
  again, to the same chunks (deterministic nonces), and are not uploaded.
- **Range reads.** `Streamed` content is `[u32 header length][header][payload]`;
  `Peer::stream_header`, `stream_range` and `read_span` fetch only the nodes
  and chunks a range touches. Only a be-block-client test uses them
  (`a_large_image_streams_without_downloading_all_of_it`). The replica and apps
  always load content whole, and `Blob` holds its payload in memory.
- **Publishing.** `publish` (`crates/be-block-protocol/wit/blocks.wit`) carries
  the full `chunks` list (every node and chunk of the manifest tree) and the full `references` list. The server's
  `store_edges` (`crates/be-server/src/blocks.rs`) deletes every edge of the
  block and inserts them again, and each `Commit` stores the full
  `references`.
- **References** come from `BlockContent::references_in` over the whole
  content.
- **Sessions.** A joining follower gets the owner's whole state in `Snapshot`.
- **Documents** save as parts chosen by their structure: list objects by id,
  large objects as parts of their own, text and other bytes by content-defined
  chunking (guides/blocks/content_types.md, "The stored form"). Typing in a
  1 MiB text block uploads one part and the nodes above it.

## The design

### 1a. Loading part of a document

Documents save as parts (guides/blocks/content_types.md, "The stored form")
but still load whole. Loading part of a document is 5; the model could later
offer it per field (an attribute such as `#[model(lazy)]` on a `List`), with a
part's objects loaded when first read. Chat's segments don't need that, since
they are its own layout.

### 3. Publish sends what changed

- `publish` carries the chunks and references added and removed since the
  expected head. The server applies the edges that changed instead of
  rewriting them all. Announcements of blocks that start or stop being
  referenced already work from changes.
- A commit stores its references in the same tree shape as the manifest, so
  unchanged references are shared too.
- `block_edges` keeps its schema.

### 4. References without loading everything

- Content that loads only part of itself can't run `references_in` over all of
  it. Each part reports its own references, and a reused part keeps the ones it
  had: the content records them in its own index (chat's segment index, for
  example) so a closed part's references never change.

### 5. Replicas and apps that load part of a block

- A type can load only part of its content: for example the `Streamed` header
  as a `Document`, with the payload fetched on demand.
- Apps and type modules get a host call that reads a byte range of the block as
  the replica loaded it, including parts not saved yet. It is a call on the
  app protocol, with no framework types.
- `Snapshot` sends only the loaded part; followers fetch the rest from the
  store.
- An operation that adds a part (closing a chat segment) carries the part's
  hash. Every replica encodes the same bytes and checks the hash.

## Order of work

3 and 4 are independent; 5 builds on 4. Chat needs 1a and 3–5; the canvas
and text need 1a and 3.

## Tests

- In be-block-client, extend
  `a_large_block_saved_after_a_small_change_uploads_only_what_changed`: the
  publish carries a small reference and object change, not the full lists.
- A replica that loads only the header reads a payload range on demand, and a
  follower joining it gets a snapshot without the payload.
