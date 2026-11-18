# Proof paths with inclusion proofs

Depends on: none. (Steps 1, 2 and 4 of the access logs checked-once work, which
this builds on, landed: `Checked::extend` and `resume`, reads after a held
entry, and the server's kept checked state.)

Set aside until a measured log size or read cost shows that re-reading a proof
path hurts (decisions/proof_paths_stay_one_step_per_entry.md).

## The problem

A reader below Edit is served a proof path (guides/blocks/keys.md, "Proof
paths"): one `Step` per entry, whole where it proves the reader's right or a
sealer's, else `Reduced`. A path is O(n) in the log's length, and it is read
and checked whole each time, since what it shows whole changes with who sealed
the reader what.

## Shape

Serve only the entries a path shows whole, with inclusion proofs against the
head, so a path is the entries naming the reader and O(log n) hashes. Done: a
reader below Edit of a log of 100,000 entries is served the entries naming it
and O(log n) hashes, and every test of `holds`, `current_at` and `outdates` on
paths passes.

Plain inclusion proofs are not enough. Today each reduced step carries the
flags its hash commits to (`Reduced`: `lowers`, `places`, `owns`), and
`check_log` reads the newest lowering, the placement and a hidden owner
transfer from them. Left out, those would be the server's word. So the tree
commits them: a Merkle sum tree whose leaf is an entry's hash with its three
flags, and whose node commits its children's hashes and a summary of its
leaves (the place of the newest lowering, of the newest parent record and of
the newest owner record, and how many owner records), each child's summary
merged into its parent's. An inclusion proof's sibling subtrees then cover
every gap between whole entries, so the reader computes each gap's summary
from them, and the head's root binds it all.

Still to settle:

- A whole entry's `previous` names an entry left out, so the chain is checked
  by place (its leaf index) rather than by hash.
- A signer's key at a whole entry's place needs every retirement of that
  signer whole, as paths serve now.
- Epochs name heads by hash, which the reader learns only for the entries it
  is sent and the head, so `holds` on another head needs that head's place.
