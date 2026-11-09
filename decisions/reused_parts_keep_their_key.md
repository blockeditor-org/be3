# A reused part keeps the key it was sealed under

A save seals only what it writes new, under the block's current data key; a
part it hands back as `Part::Reused` stays as it was, under whatever epoch
sealed it (`Vault::write`). A commit's tree therefore mixes keys after a
rotation, and a reader needs every epoch its head is sealed under: a rotation
rewraps every data key under the new scope key, and a move rewraps every epoch
the moving device trusts (`Peer::rescope`). This reverses part of #804, which
resealed every reused part under the current key on the next save.

## Why

Resealing made the first save after a rotation re-upload the whole block, so a
large block (the search index, a chat, a long blob) paid for every rotation in
full, and a rekey's forced seal rewrote everything. Sealing only what changed
keeps a save's cost in proportion to the change, rotation or not.

## What it gives up

A removed member who kept an old data key can still open the parts no save has
rewritten since, if they get the objects. The server refuses them the objects,
since they lost access, so they would need the server to collude. New content
is sealed under keys they never had.

## Rejected

- Resealing reused parts on the next save (#804): a rotation costs a whole
  re-upload of every large block it touches.
- Resealing in the background after a rotation: the same cost, only later, and
  for blocks nobody edits.
