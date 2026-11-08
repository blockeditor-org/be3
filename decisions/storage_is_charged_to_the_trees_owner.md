# Storage is charged to the owner of the tree it is in, history included

Each account has a quota in bytes (`ServerConfig::account_quota`,
`--account-quota`, 10 GiB by default; none on a device's embedded server).
Every block has a payer, stored on its row (`blocks.payer`):

- A block made inside another is paid by its parent's payer. A block at the
  top (parent `Root`, such as a My files, or `Detached` when made) is paid by
  its author, or, for an agent, by the account that created the agent.
- Example: A makes text block "1" and shares it with B to edit; B makes text
  block "2" inside "1" and writes into it. "2" counts toward A's quota, not
  B's.
- A move makes the moved subtree's payer that of where it goes: the new
  parent's payer, or, at the top, the account that moved it (an agent's
  creator). When that changes the payer, every block of the moved subtree and
  its objects are re-charged (`follow_payer`), and the move is refused when it
  would take the new payer past its quota. Moving to the trash changes
  nothing; the block stays charged until it is collected, and restoring it
  keeps its payer.
- An account is charged once for each distinct object across the blocks it
  pays for (`account_objects`), and for every object those blocks hold: each
  version they keep, not only the head, and what they hold with
  `HoldObjects` (a `put-part`'s hold only until a commit takes it or the
  upload grace passes).
- An upload is charged to its uploader's payer until one of the uploader's own
  publishes or holds uses it, or it is collected after
  `ServerConfig::upload_grace` (with the object, when no block holds it).
- Blocks report `size` (their own distinct objects) and `tree_size` (their own
  plus every child's, by the parent link).

## Why

- What people keep together is paid for together, by whoever owns the place
  it is kept: a folder's owner sees its whole size against their own quota,
  as hosted drives charge a shared folder's owner.
- A viewer can never spend anyone's storage. An editor can: anyone with Edit
  on a block, a collaborator or an organization's administrator, may publish
  into it or make blocks inside it, charged to the tree's owner. This is
  accepted as a matter of trust: Edit is granted only to contacts the owner
  chose (or comes with administering the organization), and the owner sees
  the bytes in the tree's size and can take the grant back, prune history or
  delete.
- History occupies the disk as much as the head does, so it counts; pruning
  history (`PruneHistory`) or collecting the trash is how an account frees
  space.
- Agents are made freely by members, so charging them to their creator keeps
  one person to one quota. The payer is stored when a block is made, so
  removing an agent never moves its blocks onto a fresh quota.
- At the top the mover pays, not the author: otherwise A could fill a block B
  made inside A's tree, then move it to the top and leave its bytes on B's
  quota. Making a block at the top is the same rule, since its maker is its
  mover.
- Only a move re-charges, and only the moved subtree: every other counter
  changes in O(depth).

## Rejected

- The author pays: a block made inside someone's shared folder would be
  charged to whoever made it, so the folder's owner could not see or bound
  what their folder costs (the user's example above).
- The uploader pays (whoever publishes a version pays for it): one block's
  history would be split between accounts, and someone who only edits a
  shared document would see their quota spent by its size.
- The tree's owner pays, with a breakdown per contributor and a way to prune
  what each wrote: it bounds what an editor can spend, but needs per-account
  sizes on every block and a policy for whose versions to drop. Deferred until
  editors spending an owner's quota is a problem in practice.
- Counting only heads: history would be free disk.
- Deduplicating `tree_size` across blocks: it would need a set of hashes per
  subtree. Objects are encrypted per block key, so blocks rarely share one.
