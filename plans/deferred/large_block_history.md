# Publishing and pruning by manifest node

Depends on: none.

Set aside from the follow-ups to #804 (merging by parts, per-type retention,
reused parts keeping their key, all built): nothing needs it yet, and it rewrites
the server's schema, access checks and billing.

The server writes a `commit_objects` row for every object of every commit it
keeps (`be-server/src/blocks.rs`, publish) and reads the expected head's whole
list, so a publish to a 1 GB block costs thousands of rows whatever changed,
and history costs rows in proportion to size times states kept. The idea: each
new node is sent with the hashes of its children (the server learns the tree's
shape, which object sizes and timings mostly show already), nodes are
refcounted, a commit holds only its root, and reclaiming cascades through a
node's children when its count reaches zero. A publish and a prune then cost
the changed nodes, about depth times changed parts. The client's outbox keeps
whole object lists too and needs the same change.

What it would have to replace, since `commit_objects` drives more than
reclamation:

- read access to objects: `holders_of` decides who may fetch a hash;
- per-payer charging: `block_objects`, `held_by_block`, `held_in_organization`
  in `store/usage.rs`;
- `revoke_or_grant`;
- `read_contents`, which answers the head's whole object list.

Each needs a walk up the tree ("which blocks hold this node"). The server
cannot check the child hashes a client claims, and sealed objects repeat
within a key (deterministic nonces), so a client lying about a node's children
could get objects other blocks share reclaimed; decide how far the server
trusts the claims. decisions/block_reclamation_by_reference_counting.md ("a
commit holds its chunk hashes") changes with it.
