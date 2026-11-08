# Large blocks, edited and merged

Follow-ups to #804 (large blocks and saving live edits less often), so a block
of a gigabyte or more can be edited often, merged and kept with history at a
cost in proportion to what changed. The search index (`plans/search.md`) is
the first such block; blobs and chat (`plans/chat.md`) gain from the same
changes.

- **`merge3` over parts.** Today it takes each side's whole bytes. It should
  take each side as `loaded` (the first part's bytes and the part list), read
  other parts with `read-part` while it runs, and answer `written` with
  `reused` parts, so a merge reads and uploads only what differs. Documents
  keep merging whole bytes until they load by parts.
- **Lazy rekeying.** `Vault::write` reseals every reused part sealed under an
  older key (`Vault::stale_parts`, from #804's fix to taking a share back), so
  after a rotation the next save of a large block re-uploads all of it. Keep a
  reused part under the epoch it was sealed with instead, and seal with the
  current key only what a save writes new, part by part (a part over the
  chunker's minimum is several chunks; keeping each leaf near one chunk makes
  that per chunk). A block keeps every epoch, so readers lose nothing. What it
  gives up: a removed member who kept an old key can read the unchanged parts
  if the server also serves them, which the server already refuses and could
  only do by colluding. A subtree then mixes keys, so `Vault::sealed_under`
  and the guide's "a subtree is always sealed under one key" change, and a
  rekey's forced seal writes only the parts it changes. Record it as a
  decision, since it reverses part of #804.
- **Publishing and pruning cost what changed.** The server writes a
  `commit_objects` row for every object of every commit it keeps
  (`be-server/src/blocks.rs`, publish) and reads the expected head's whole
  list, so a publish to a 1 GB block costs thousands of rows whatever changed,
  and history costs rows in proportion to size times states kept. Huge blocks
  with history need the server to know the manifest tree's shape instead: each
  new node sent with the hashes of its children (the server learns the tree's
  shape, which object sizes and timings mostly show already), refcounts on
  nodes, a commit holding only its root, and reclaiming cascading through a
  node's children when its count reaches zero. Then a publish and a prune cost
  the changed nodes, about depth times changed parts. The client's outbox
  keeps whole object lists too and needs the same change.
- **Per-type retention.** Retention (`RetentionPolicy`) is per organization.
  A type should be able to ask for a shorter one (the search index keeps a
  few recent states), which the server applies by content type.
- **An operation that adds a part by its hash.** Chat's replicas encode the
  part themselves and check it against the hash (`plans/chat.md`); the search
  index's writer uploads the part first and other replicas reuse it unread.
  One operation serves both: the part is held by hash, and a replica either
  rebuilds it or takes it as `reused`.

## Order of work

1. Publishing and pruning by node, since every other change makes large
   blocks more common.
2. Lazy rekeying, with its decision record.
3. `merge3` over parts.
4. Per-type retention.
5. The operation that adds a part by its hash.
