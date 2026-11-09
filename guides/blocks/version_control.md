# Version control

Part of the block stack (guides/the_new_block_stack.md). Version control is two
content types and a set of peer-side operations, not a store of its own:
history is be-block-store objects and be-block-commit commits. The model is
Jujutsu's: the working copy is a commit, a change keeps its identity when
rewritten, conflicts are data, and every operation can be undone.

## The pieces

- A **repository** (`be_block::Repository`) holds branch heads, an optional
  upstream repository in the same organization, and the operation log. It
  keeps its history alive with `Peer::hold` (`HoldObjects`, `Hold::Lasting`), whatever its own
  commits' retention does.
- A **snapshot** is a `Commit` of kind `Snapshot(description)` whose manifest is
  a `be_block_vcs::Tree`: one entry per block of the versioned subtree, keyed by
  local id (type, parent, metadata, references, content manifest), plus the
  tree's unresolved conflicts. Entries reuse the blocks' own manifests, so an
  unchanged block stores nothing new and a checkout publishes a first commit
  without downloading it.
- A **change** is the line of snapshots sharing a `ChangeId`
  (`Commit::change`); rewriting keeps the change id and makes a new commit id.
  A commit with none is its own change (`Commit::change_id`).
- A **checkout** (`be_block::Checkout`) is the parent of a working tree of
  ordinary blocks, edited by every editor and session as usual. Sharing the
  checkout block is working together; two checkouts are working apart. It
  records its repository, branch, `working` snapshot, root, each block's head
  at that snapshot (so status compares against the mirror, not a download), and
  views of conflicts. There is no staging area: edits are the working change,
  every operation first snapshots it, and `Finish` describes it, moves the
  branch by compare-and-swap and starts a new empty change.
- **Conflicts are data.** `be_block_vcs::merge` records each conflict in the
  merged tree with each side's entry and carries existing ones until resolved.
  Bringing changes in, switching branch (rebasing the working change) and
  combining a branch (a second parent) never refuse and never leave anything
  in progress. `Tree::resolve` is an ordinary rewrite.
- **The operation log** (`Repository::operations`, the latest
  `OPERATION_LIMIT`) records each operation's action, checkout and before/after
  views. Undo is an operation: it puts back branches that have not moved since
  and reverts the working change by a three-way merge, so later edits stay.
  Undoing an undo redoes.

## Scopes and local ids

Content stores the ids of the blocks it names, and those have to mean the same
in every checkout, so they are *local* ids. A block copied into a checkout is
created as `masked(local, scope_mask(checkout))` with the `LOCAL_ID` metadata
key; a block created in or adopted into a checkout keeps its own id as its
local id. The host translates every block id crossing the app protocol for an
instance inside a checkout (`Instances::translate`, over the `BlockIds` of what
crosses, in `be-host-runtime/src/calls.rs`), and the peer resolves published
references the same way (`Peer::set_resolver`), so neither content nor editors
know about checkouts. Resolution looks in the instance's checkout first and
falls back to the block with that id, so references out of the tree work.

## Operations

`crates/be-host-stack/src/version.rs` runs them in the worker: adopt, save
(finish), checkout, switch, branch, combine, bring in (`be_block_vcs::merge`
over the trees, then `Replicas::merge` for blocks both sides changed, which
is each type's `Merge::merge3` or, for a type that merges by parts, its
`MergeParts` (guides/type_modules.md, "Merging by parts"), leaving `before`, `yours` and `incoming` copies of a conflicted block
under the checkout), resolve, undo, fork, pull and push. It answers a
`version-watch` with each shown repository's and checkout's `version-status`,
recomputed when the mirror changes, and holds followed checkouts' trees in the
mirror (`Query::Subtree` under `Owner::Version`), re-listing before each
operation so every head is current.

A checkout replaces content someone has open through the replica's reconcile
on `head-moved` (the block's `watch` stream's `moved`); the worker calls
`head-moved` with `from-this-device` for what it publishes itself.

## Not built yet

- A snapshot takes each block's head after this peer seals what it owns, so
  another peer's unsealed session edits are left out; a seal barrier would fix
  that.
- Conflicts resolve a block at a time by taking one side; the three sides are
  linked, not shown side by side.
- A repository never lets go of history until it is collected itself; freeing
  a branch needs the server to count holds per object.
- An upstream must be in the same organization: across organizations the keys
  differ and a pull would reseal every object. Reading a repository needs the
  data keys of every block it snapshotted, since trees reuse their manifests.
- Moving a block into or out of a checkout keeps the ids its content stores, so
  what they name changes with the scope it lands in.
