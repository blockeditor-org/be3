# Proof paths stay one step per entry

A proof path (guides/blocks/keys.md, "Proof paths") keeps a `Step` for every
entry of the log, whole or reduced, and a reader below Edit reads and checks it
whole each time. Serving it as the whole entries alone with Merkle proofs is
set aside (plans/deferred/proof_paths_with_inclusion_proofs.md).

## Why

Access logs grow with grants, share link openings and retirements, not with
content, so they stay small. Devices already check each entry of a whole log
once, read only what was appended, and the server checks an append alone, so
re-reading a log is cheap where it is long. A Merkle sum tree that commits what
a reduced step says today (whether it lowered anyone, moved the block or
changed its owner) adds a second structure to keep and to trust, for a gain no
workload shows yet.

Revisit it when a measured log size or read cost shows re-reading a proof path
hurts.

## Rejected

- **Plain inclusion proofs** of the whole entries against the head: the
  entries left out would no longer say whether they lowered anyone, moved the
  block or changed its owner, so a reader below Edit would take its newest
  lowering, its placement and a hidden transfer on the server's word, which it
  checks today.
