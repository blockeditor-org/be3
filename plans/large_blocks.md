# Large blocks

A block loads, saves and publishes whole, so every save costs more as the block
grows, whatever changed. This plan makes a save cost what changed, and lets a
reader load part of a block. Chat needs it (plans/chat.md); the canvas, and any
block with many children or attachments, gains too. Saving less often is
plans/live_edit_persistence.md. Not implemented yet.

## What exists

- **Chunking.** `be_block_store::chunker::split` is content-defined:
  `ChunkerConfig::DEFAULT` is 256 KiB minimum, 1 MiB average, 4 MiB maximum.
  The chunker is set per `Vault` (`with_chunker`), not per content type. A
  commit holds a flat `Manifest { content_type, length, chunks: Vec<ChunkRef> }`.
- **Saving.** `CommitStore::write(…, data: &[u8], …)` chunks the whole stored
  form again on every save. Unchanged chunks deduplicate, but an edit anywhere
  re-uploads the chunk around it, at least 256 KiB.
- **Range reads.** `Streamed` content is `[u32 header length][header][payload]`;
  `Peer::stream_header`, `stream_range` and `read_span` fetch only the chunks a
  range touches. Only a be-block-client test uses them
  (`a_large_image_streams_without_downloading_all_of_it`). The replica and apps
  always load content whole, and `Blob` holds its payload in memory.
- **Publishing.** `publish` (`crates/be-block-protocol/wit/blocks.wit`) carries
  the full `chunks` list and the full `references` list. The server's
  `store_edges` (`crates/be-server/src/blocks.rs`) deletes every edge of the
  block and inserts them again, and each `Commit` stores the full
  `references`.
- **References** come from `BlockContent::references_in` over the whole
  content.
- **Sessions.** A joining follower gets the owner's whole state in `Snapshot`.
- Commits, manifests and chunks are server data, so they are frozen
  (guides/frozen_formats.md).

## The design

### 1. Content chooses its chunks

- `BlockContent` may return its stored form as parts. A part is either new
  bytes, saved as a chunk of its own, or an existing `ChunkRef` from the head it
  was loaded from. By default content returns one part holding all its bytes,
  chunked as today.
- The vault seals new parts and writes a manifest that reuses the old refs.
  Seal nonces are derived from the key and the plaintext
  (`decisions/deterministic_seal_nonce.md`), so two replicas that encode the
  same part get the same chunk and hash.
- `Blob` puts its header in a part of its own, so changing the header no
  longer re-chunks the payload.
- **(ask)** whether the canvas splits its stored form into parts (by layer, or
  by region), and how.
- After key rotation a manifest mixes chunks sealed under different data key
  epochs. Sealed bytes name their key, so reading should work; add a test.

### 2. A manifest tree

- The commit names a root object; the manifest is a tree of objects, and
  unchanged subtrees are shared between commits. A commit then costs its
  changed parts plus a few small nodes, not a list of every chunk.
- The flat manifest is frozen: keep reading it forever, and add the tree as a
  new form beside it.

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

1, 3 and 4 are independent; 2 builds on 1; 5 builds on 1 and 4. Chat needs all
five.

## Tests

- In be-block-client: a large block saved after a small change uploads one
  small chunk and a few manifest nodes, and publishes a small reference change.
  Count bytes with the fault relay.
- A manifest mixing data key epochs reads after a rotation.
- A flat manifest saved by an earlier build still reads (be-block-frozen's
  snapshots).
- A replica that loads only the header reads a payload range on demand, and a
  follower joining it gets a snapshot without the payload.
