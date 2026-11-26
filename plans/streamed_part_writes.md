# Streamed part writes

Depends on: none.

The vault and the peer write a part a piece at a time already: `Vault::part_writer` seals each
chunk as the bytes end it (`chunker::Splitter` cuts what `split` would), holding at most one
chunk, and answers the `PartRef` a whole write of the same bytes would, so replicas and devices
still agree; `Peer::part_writer` hands one under a block's sealing key, `Peer::hold_part` has
the server hold what it wrote for the block, and `Peer::put_part` is the two over whole bytes.
Left: the type protocol's `put-part` still takes `list<u8>`, and the search index's merge
(`be_lib_search`'s `compact::merge_segments`) reads its inputs whole (`Meta::read_all`,
`Meta::documents`) and builds its output whole (`segment::build`), which is why the idle pass is
capped at `IDLE_MERGE_BYTES` (plans/backlog.md, 11, "Idle compaction merges larger tiers").

The aim: the segment merge reads its inputs a block at a time and writes its output parts
through part writers as it goes, so the idle pass merges any tier; and a type module can stream
a part it puts too.

## Design

- **Type protocol.** `host.put-part` becomes a resource, `part-writer` (created on the replica
  with `put-part()`), with `write: async func(bytes: list<u8>)` and
  `finish: async func() -> result<part-ref, string>`; nothing in it names a framework, a frame
  or an editor. be-block-type-host serves it with a `Peer::part_writer`, holding the part at
  `finish` (`Peer::hold_part`); be-sdk-type-guest's `Effect::PutPart` becomes a writer the replica feeds, and
  be-sdk-test's store keeps refusing. An editor's `FromEditor::PutPart` still sends whole bytes
  (an app frame is at most 8 MB), so it is fed as one write.
- **Search sinks.** be-lib-search gains `PartsOut`, the write side of `Parts`: `create(name)`
  answers a writer for a new part (`write`, `finish`), and `Written::added` holds each added part
  as `Added::Bytes(Vec<u8>)` or `Added::Saved` (the length, and a ref the caller's sink keeps by
  name), so small writes (a batch's segment, a deletions bitmap, the table) stay bytes and a
  merge's output can be saved as it goes. be-block-search-index implements it with an
  `AsyncWrite` beside `AsyncRead`, `Held::Saved` taking what the sink saved; `SetIndex` and
  saving are unchanged.
- **Streaming merge.** `merge_segments` reads each input a block at a time and writes the four
  output parts in order:
  - *Documents*, two passes over the inputs' documents parts, read a window of records at a
    time: each input's records are already in local order (newest first, then by `DocId`), so a
    k-way merge gives the output order. The first pass keeps the live ones (the table points at
    them), builds each input's map from its local id to the new one (a `u32` per document) and the
    new offsets (a `u32` per document); the second writes the count, the offsets and the records.
  - *Dictionary, postings, positions*, a k-way merge over the inputs' dictionaries one block at a
    time: each term reads its postings and positions from each input holding it by range,
    remaps and drops documents, and is written to the three outputs (a term's skip data comes
    before its body, so one term's postings are held at once).
  - The table entries move as today (`table::Edit`, held per moved document).
  Memory is then O(documents) small integers plus one term's postings and the read windows, not
  the inputs' bytes. The output is byte for byte what `build` writes from the same documents and
  terms, so the existing compaction tests hold.

## Steps

1. **Type protocol part writer.** The `part-writer` resource in be-sdk-type-api's `host.wit`,
   served by be-block-type-host, used by be-sdk-type-guest's replica for `Effect::PutPart`.
   Done: `an_operation_adds_a_part_by_its_hash` and be-host-stack's
   `a_part_an_editor_puts_is_held_for_its_block_once_the_server_has_it` pass over the writer,
   and a be-block-type-host test writes a part in several pieces and reads it back.
2. **Search sinks.** `PartsOut` and `Added` in be-lib-search, `AsyncWrite` in
   be-block-search-index, with compaction writing its merged segment's parts through the sink
   (still built whole). Done: be-block-search-index tests compact with a sink that saves parts
   and get the same index and search hits as with bytes.
3. **Streaming merge.** `merge_segments` as above. Done: a be-lib-search test merges segments
   through `Parts` that refuse any read longer than a dictionary block or one term's postings and
   a sink that records the most bytes held, both under a bound much smaller than the inputs,
   giving the same parts as the whole merge did.
4. **The idle pass merges any tier.** `compact_past` writes through a `Peer::part_writer` sink,
   and `IDLE_MERGE_BYTES` goes (the idle pass merges whatever the tiers want). Done: a
   be-host-stack test compacts an index whose merge is larger than the old cap, at rest, and
   the backlog item is deleted.
