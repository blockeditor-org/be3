# Large blocks, edited and merged

Depends on: none.

Follow-ups to #804 (large blocks and saving live edits less often), so a large
block can be edited often and merged at a cost in proportion to what changed.
The search index (`plans/search.md`) is the first such block; blobs and chat
(`plans/chat.md`) gain from the same changes. Publishing and pruning by
manifest node, which huge blocks with long history need, is
`plans/deferred/large_block_history.md`.

Every test here is at small scale: a block of a few parts, counting what is
read, uploaded or written, never a gigabyte.

## 1. An operation that adds a part by its hash

Chat's replicas encode a closed segment themselves; the search index's writer
uploads a segment first; a text edit over the relay limit
(plans/text.md, step 3) uploads its payload and is relayed by hash. One
operation serves all three: the uploaded bytes may be a part of the stored
form or an operation's payload, which each replica reads with `read-part` and
applies.

- **WIT** (`be-sdk-type-api`): the module's `written` already keeps a part by
  `reused(part-ref)`. Add a way for an operation to name a part by its hash: a
  host call `put-part(bytes) -> part-ref` that seals and uploads a part under
  the block's current key before the operation is relayed, and a
  `part-ref` field the type puts in its own operation payload.
- **Flow.** The sender calls `put-part`, waits for the upload, then sends the
  operation carrying the `part-ref`. Each replica that applies it either takes
  the part as `reused` without reading it (the search index), or rebuilds the
  bytes itself and checks them against the hash (chat), reading the part with
  `read-part` when its own encoding differs (another build can encode
  differently).
- **Held by the block.** The server holds an uploaded part for the block from
  the upload (`HoldObjects`, charged as any upload) until a commit that uses it
  publishes or the upload grace runs out.
- **Done**: a test type sends an operation adding a part; a second replica
  saves a commit whose manifest reuses that part without uploading it again;
  a replica that rebuilds different bytes falls back to `read-part` and ends
  with the same content.

## 2. `merge3` over parts

Today `merge3` takes each side's whole bytes (`replicas.wit`), and the Rust
`Merge::merge3(&Self, &Self, &Self)` is synchronous over whole values.

- Add a part-aware, async merge beside it: each side as `loaded` (the first
  part's bytes and the part list), answering `written` with `reused` parts and
  `conflicts` as today's `merged` does. `Merge::merge3` stays for whole values;
  a type that loads only its first part implements the new one, and the host
  calls whichever the type has.
- **`read-part` under stateless requests.** A host call without an open replica
  as its parent is refused today (`Heard::Call` in
  `be-block-type-host/src/replicas.rs`). Serve `read-part` under a stateless
  request too, scoped to the commits that request named, in both hosts:
  `Replicas::merge` and `Modules::merge3` (`be sync`, `be-block-sync/src/pass.rs`).
  `search-text` (`plans/search_library.md`) uses the same change.
- Documents keep merging whole bytes until they load by parts
  (`plans/backlog.md` item 5, Loading part of a document).
- **Done**: a test type with ten parts merges two sides that each changed one
  part, reads only those parts and the base's, and publishes a commit that
  reuses the other eight; a stateless request reading a part of a commit it
  did not name is refused; `be sync` merges the same type.

## 3. Per-type retention

Retention (`RetentionPolicy`) is per organization. A server-side table maps a
content type to a shorter policy (the search index keeps a few recent states),
set when the type is registered with the server or by an administrator; the
server runs no type modules, so it learns nothing else. A type's policy never
keeps less than the organization's `history_kept_for`.

- **Done**: a server test publishes a run of commits to a block of a type with
  a short policy and one without; the first keeps only its policy's states,
  the second the organization's, and both keep everything inside
  `history_kept_for`.

## 4. Lazy rekeying

`Vault::write` reseals every reused part sealed under an older key, so after a
rotation the next save of a large block re-uploads all of it. Keep a reused
part under the epoch it was sealed with, and seal with the current key only
what a save writes new, part by part (keeping each leaf near one chunk makes
that per chunk).

- **Prerequisite: rewrap every old data key on a move.** A move into another
  scope does not rewrap old data keys a device cannot open
  (guides/blocks/keys.md, "Not built yet"); today that only closes old
  history, but with lazy rekeying the head needs those keys, so a moved block
  would become unreadable in its new scope. Rewrap every epoch the block keeps
  first.
- Delete `Vault::stale_parts`, `reseal` and the stale-part fetch in
  `Peer::write_content`. A subtree then mixes keys, so `Vault::sealed_under`
  changes, and a rekey's forced seal writes only the parts it changes.
- What it gives up: a removed member who kept an old key can read the
  unchanged parts if the server also serves them, which the server refuses and
  could only do by colluding.
- Update guides/blocks/server.md ("a subtree is always sealed under one key")
  and guides/blocks/keys.md ("reseals the parts it reuses"), and record a
  decision, since it reverses part of #804.
- **Done**: after a rotation, saving a ten-part block that changed one part
  uploads one part and its nodes; a viewer with only the new epoch's wrap
  still reads every part; a block moved into another scope and then saved
  stays readable to the new scope's members.

## Order of work

1. The operation that adds a part by its hash.
2. `merge3` over parts, with `read-part` under stateless requests.
3. Per-type retention.
4. Lazy rekeying, after rewrapping old keys on a move.
