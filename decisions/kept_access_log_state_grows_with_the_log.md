# Kept access log state grows with the log

What a device and the server keep of a checked access log (`Checked`) grows
with the log: `positions` keeps every entry's hash, and `levels` every level
change by place. It is left unbounded.

## Why

Epochs name their log heads by hash (`CarriedKey::heads`, `EscrowKey::head`),
and `holds`, `current_at`, `outdates` and `level_as_of` judge a key by any head
it names. A device that kept only an accumulator over the entries' hashes
would need an inclusion proof for each head from the server, since it keeps no
inner nodes; and the levels as of a head would need a commitment to the log's
state at every head, which no inclusion proof gives. That is a protocol change
on both sides, for logs that grow with grants and retirements, not with
content. Each entry is already checked once and, in the refs, written once
(`refs::kept_logs`), so a long log costs memory and a read at startup, not
work per append.

Revisit it when a measured access log size hurts.

## Rejected

- **Heads named by place with server-served inclusion proofs and a state
  commitment per head.** Bounds the kept state, for a new query, a Merkle tree
  of each access log on the server and a state commitment scheme.
- **Pruning before the oldest head a held key names**, a key naming an older
  head taken as outdated. No protocol change and it fails closed, but it bounds
  the state only by how long epochs live, and rotates keys that arrive late.
- **Keeping only the entries since the last lowering.** A log of grants alone
  never lowers.
