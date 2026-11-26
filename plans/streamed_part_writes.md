# Streamed part writes

Depends on: none.

The vault, the peer and the type protocol write a part a piece at a time already:
`Vault::part_writer` seals each chunk as the bytes end it (`chunker::Splitter` cuts what `split`
would), holding at most one chunk, and answers the `PartRef` a whole write of the same bytes
would, so replicas and devices still agree; `Peer::part_writer` hands one under a block's sealing
key, `Peer::hold_part` has the server hold what it wrote for the block, and a type module's
`host.part-writer` is served by the two (guides/type_modules.md, "Parts named by hash"). The
search index's merge streams through them (guides/blocks/host.md, "Compaction"). Left: what a
part writer sealed and no commit came to name stays in the vault.

The aim: a part sealed on the device that nothing names is let go.

A writer takes its block's sealing key once, as it starts (`Peer::part_writer`), so a key that
rotates mid-write seals the whole part under the old key, as a whole put begun just before the
rotation would. A commit that reuses the part seals it again under the key it is written under
(`Peer::write_content_under` fetches and rewrites a stale part), but until one does, the part
the operation names is under the old key, which a member the rotation removed still holds. A
module's writes are short-lived and the server's access checks still stand, so nothing handles
it yet; if it comes to matter, `finish` can fail when the block's key moved since the writer
started, and the module put the part again.

## Steps

1. **Unnamed parts.** A compaction seals each merged part into the local vault as it writes
   it (be-host-stack's `Saving`), before any commit names it. When the commit is never made, the
   parts stay: `compact_past` let go of (the head moved, a read failed), `write_at` failing after
   its compaction, or a host stopping mid-merge. A type module's `part-writer` dropped before
   `finish`, or one whose part no operation came to name, leaves its chunks the same way.
   Nothing removes an object no commit names, and
   since sealing is convergent (the same bytes under the same key are the same chunks), another
   part may share a chunk, so deleting a part's chunks needs a count of what names each. The
   step: a local collection of the vault's objects that no held commit, outbox entry or part
   writer still open names (or a reference count kept as parts are sealed and let go of), run
   while idle. Done: a be-host-stack test lets a compaction go after it sealed parts, and the
   vault holds none of them after the collection, while every part of the index's head reads
   back.
