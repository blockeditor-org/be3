# Streamed part writes

Depends on: none.

The vault and the peer write a part a piece at a time already: `Vault::part_writer` seals each
chunk as the bytes end it (`chunker::Splitter` cuts what `split` would), holding at most one
chunk, and answers the `PartRef` a whole write of the same bytes would, so replicas and devices
still agree; `Peer::part_writer` hands one under a block's sealing key, `Peer::hold_part` has
the server hold what it wrote for the block, and `Peer::put_part` is the two over whole bytes.
The search index's merge streams through them (guides/blocks/host.md, "Compaction"). Left: the
type protocol's `put-part` still takes `list<u8>`, so a type module holds a part it puts whole.

The aim: a type module can stream a part it puts.

## Design

- **Type protocol.** `host.put-part` becomes a resource, `part-writer` (created on the replica
  with `put-part()`), with `write: async func(bytes: list<u8>)` and
  `finish: async func() -> result<part-ref, string>`; nothing in it names a framework, a frame
  or an editor. be-block-type-host serves it with a `Peer::part_writer`, holding the part at
  `finish` (`Peer::hold_part`); be-sdk-type-guest's `Effect::PutPart` becomes a writer the
  replica feeds, and be-sdk-test's store keeps refusing. An editor's `FromEditor::PutPart` still
  sends whole bytes (an app frame is at most 8 MB), so it is fed as one write.

## Steps

1. **Type protocol part writer.** For a type module that puts a part larger than it wants to
   hold (the host compacts search indexes with `Peer` directly). The `part-writer` resource in
   be-sdk-type-api's `host.wit`, served by be-block-type-host, used by be-sdk-type-guest's
   replica for `Effect::PutPart`. Done: `an_operation_adds_a_part_by_its_hash` and
   be-host-stack's `a_part_an_editor_puts_is_held_for_its_block_once_the_server_has_it` pass
   over the writer, and a be-block-type-host test writes a part in several pieces and reads it
   back.
2. **Unnamed parts.** Independent of step 1. A compaction seals each merged part into the local vault as it writes
   it (be-host-stack's `Saving`), before any commit names it. When the commit is never made, the
   parts stay: `compact_past` let go of (the head moved, a read failed), `write_at` failing after
   its compaction, or a host stopping mid-merge. Nothing removes an object no commit names, and
   since sealing is convergent (the same bytes under the same key are the same chunks), another
   part may share a chunk, so deleting a part's chunks needs a count of what names each. The
   step: a local collection of the vault's objects that no held commit, outbox entry or part
   writer still open names (or a reference count kept as parts are sealed and let go of), run
   while idle. Done: a be-host-stack test lets a compaction go after it sealed parts, and the
   vault holds none of them after the collection, while every part of the index's head reads
   back.
