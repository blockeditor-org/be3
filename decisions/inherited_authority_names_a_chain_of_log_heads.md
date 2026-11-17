# Inherited authority names a chain of log heads

A record signed on a right from a block above (`Authority::Inherited`) names
the chain of blocks up to the block it rests on, each as a `Link` with a head
of that block's log: the first is where the signed block lay at the record's
place, and each next is where the one before lay as of its head. Each log is
checked from the root the placement under it names, never from the server's
tree. The server takes the record only while every head is current, so the
record checks the same way however the blocks between move later
(guides/blocks/keys.md, "Parent records" and "Accepted limits", "Stale
authority").

## Why

Inherited access is the highest grant on the path up
(decisions/block_access_is_the_highest_grant_on_the_path_up.md), so a
manager above may grant or move below. A device has to check that right
without taking the server's word for where a block lies, and it has to get
the same answer later, after moves.

## Rejected

- **Trusting the server's tree.** The server could place a block under an
  ancestor that someone it chose manages, and so seal the block's keys to
  them.
- **The blocks between as they ever lay.** Counting any block the signed
  block once lay under let a later manager of a former ancestor sign for it.
  Placements as of named heads count only where the block lay at that moment.
- **A vouch per account** (an account once found managing above is trusted
  from then on). It outlived a revocation above. The chain is checked as of
  the heads it names, and a device vouches only for one wrap as it arrives
  (`KeyBook::vouch`).
