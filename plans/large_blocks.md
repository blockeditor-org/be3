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

Step 1, an operation that adds a part by its hash (`put-part`), is built
(guides/type_modules.md, "Parts named by hash"); the other steps keep their
numbers, which other plans cite.

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
content type to a shorter policy (the search index keeps a few recent states).
The table is built into the server, next to the content-type constants it
already knows (`SETTINGS_TYPE`, `SEARCH_INDEX_TYPE` in `be-block-graph`): the
server runs no type modules and has no type registration, so no protocol
call or setting fills it. A type's policy never
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
  would become unreadable in its new scope. Rewrap, on a move, every epoch the
  head's parts are sealed under (a device that can read the head holds them);
  older epochs stay closed to the new scope, as today. Done: a test rotates a
  block, saves without resealing, moves it into another scope, and a member
  of the new scope reads the head.
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

2. `merge3` over parts, with `read-part` under stateless requests.
3. Per-type retention.
4. Lazy rekeying, after rewrapping old keys on a move.
