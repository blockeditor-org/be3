# Storage is charged to a block's author, history included

Each account has a quota in bytes (`ServerConfig::account_quota`,
`--account-quota`, 10 GiB by default; none on a device's embedded server). A
block's objects are charged to the account that authored it, or, for an
agent, to the account that created the agent; the payer is fixed in
`block_sizes` when the block first holds anything. An account is charged once
for each distinct object across the blocks it pays for, and for every object
those blocks hold: each version they keep, not only the head, and what they
hold with `HoldObjects`. An upload no block holds yet is charged to its
uploader until a block takes it, and is deleted after
`ServerConfig::upload_grace`. Blocks report `size` (their own distinct
objects) and `tree_size` (their own plus every child's, by the parent link).

## Why

- A shared folder's bytes count once, each file to whoever made it, and never
  to someone who only received a share: nobody fills another account by
  sharing into it, which is how hosted drives charge (the owner pays).
- The author never changes, so moving a block, even into someone else's
  folder or the trash, re-charges nothing: every counter moves in
  O(depth), and none needs a walk of the subtree.
- History occupies the disk as much as the head does, so it counts; pruning
  history (`PruneHistory`) or collecting the trash is how an account frees
  space.
- Agents are made freely by members, so charging them to their creator keeps
  one person to one quota.

## Rejected

- The account whose My files holds the block: a move between workspaces
  would re-charge the whole moved subtree, object by object, and a block in a
  company workspace has no such account.
- The account editing: whoever publishes a version of a shared block would pay
  for that version, splitting one block's history between accounts.
- Counting only heads: history would be free disk.
- Deduplicating `tree_size` across blocks: it would need a set of hashes per
  subtree. Objects are encrypted per block key, so blocks rarely share one.
