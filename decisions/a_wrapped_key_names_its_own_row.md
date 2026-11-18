# A wrapped key names its own row

What a key is sealed or wrapped as (`be_block_keys::CarriedKey`) carries its own
row as its maker made it: its id, kind and epoch (`Label`), beside the block it
is a key of, its root and its heads. A device takes a key only when that label
is the row the server serves it under (`KeyBook::opened`), and the server, which
opens the settings' keys, checks the same.

## Why

A wrap under a scope key names only the key it is wrapped under. The server
could copy the wrap of an old data key to a row of its making, a newer epoch of
the same block, and have a device that rotated write under the old key again.
Bound inside the seal, the row cannot be changed by anyone who never held the
key.

## Rejected

- The key's id alone: kind and epoch are what a device chooses a key by, and
  carrying them costs nine bytes.
- Binding the row as the seal's associated data: what a key is wrapped as is
  opened by several paths (a seal to an account, an escrow wrap, a wrap under a
  key), and the label inside reaches all of them the same way, as the block and
  heads already do.
