# A part reused from the same block keeps the key it was sealed under

A save seals only what it writes new, under the block's current data key. A
part it hands back as `Part::Reused` that one of the block's own epochs sealed
stays as it was (`Vault::write`). A part reused from another block (a checkout
filled from another's snapshot, a merge reusing its sides' parts) is sealed
again under this block's key (`Peer::write_content`, `Vault::adopt`), so no
block needs another block's key to be read.

A commit's tree therefore mixes the block's epochs after a rotation, and a
reader needs every epoch its head is sealed under. A rotation rewraps every data
key under the new scope key, and a move rewraps every epoch not yet wrapped
under its destination (`Peer::rescope`, which walks the whole subtree). Both are
refused (`ClientError::Untrusted`) by a device that does not trust every epoch,
such as an administrator's holding them only through escrow, which leaves the
work to one that does. A type that writes whole bytes is unaffected: every save
seals all of it again.

This reverses part of #804, which resealed every reused part under the current
key on the next save.

## Why

Resealing made the first save after a rotation re-upload the whole block, so a
large block (the search index, a chat, a long blob) paid for every rotation in
full, and a rekey's forced seal rewrote everything. Sealing only what changed
keeps a save's cost in proportion to the change, rotation or not.

## What it gives up

- A removed member who kept an old data key can open every part of the current
  head that no save has rewritten since, not only history up to the removal,
  if they get the objects. The server refuses them the objects, since they lost
  access, so they would need the server to collude (guides/blocks/keys.md, what
  the server is trusted for).
- A device that has not yet learned of a rotation (or that a colluding server
  keeps unaware of it) seals new content under the old key. Later saves and
  merges reuse that part as it is, so it stays readable with a key the removed
  member holds until a save rewrites it. Neither the device nor the server
  checks a publish's key against the newest epoch.
- A block whose old epoch no device trusts any more (its wraps lost) cannot be
  rotated or moved into another scope until it is rewritten.

## Rejected

- Resealing reused parts on the next save (#804): a rotation costs a whole
  re-upload of every large block it touches.
- Resealing in the background after a rotation: the same cost, only later, and
  for blocks nobody edits.
- Keeping a part reused from another block under that block's key: a checkout
  shared alone, or kept after its source was deleted, could not be read.
- Letting a device that does not trust an epoch rotate anyway and skip that
  key: the head's parts under it would be closed to everyone let in later.
