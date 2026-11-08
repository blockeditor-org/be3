# Unfiled

Depends on: none.

A place for blocks made without a place the person chose, so My files only
ever holds what they put there themselves. Not implemented yet.

## What exists

- My files is a root per account per organization with a derived id
  (`be_block::my_files(organization, account)` in
  `crates/be-block/src/organization.rs`), kept by the server for that account
  alone and made on opening an organization (`RootSettings::ensure_my_files`
  in `be-host/src/root_settings.rs`).
- Apps name where a new block goes with `block-location` (`root`, `detached`,
  `block(id)`, `crates/be-sdk-api/wit/types.wit`); a pick's `place` is
  optional.
- guides/blocks/workspaces.md: every picker places a new block in a
  workspace, My files when none is named. So blocks the person never placed
  show up in My files.
- A root block is its own key scope (guides/blocks/keys.md).

## The design

- **The block.** A second derived root per account per organization,
  `be_block::unfiled(organization, account)`, made and kept like My files, in
  a scope of its own. Administrators of a company organization see it as they
  see My files.
- **Liveness.** Unfiled has no content listing its blocks: like `Root`, it
  holds every block whose parent names it (`BlockGraph::holds`), so making a
  block there edits nothing anyone else must be able to edit. Taking one out is
  a move; deleting one moves it to Recently Deleted.
- **Placing.** `block-location` gains `unfiled`, which the host resolves to the
  account's own Unfiled; apps never learn its id. A connected app
  (plans/deferred/connected_apps.md) creates there through the server, and that plan
  covers how its keys are wrapped. Everything made without a
  place the person chose goes there: a pick with no place, a terminal an app
  makes for a connection (plans/boat.md), an exported dynamic artifact, a
  recording, a file pasted or dropped where nothing takes it, a file shared to
  be-host on Android. Audit every call site that creates a block against this.
- **The files pane.** A section below My files: a flat list, newest first.
  Each row names the app that made the block and where it is used, from its
  backrefs ("boat.dev · used in Dev VMs"). No folders are made or arranged
  there; the one thing to do is move a block out, which files it. References
  are by id, so filing breaks nothing.
- **Leftovers.** A block nothing references shows "Not used anywhere", and the
  section offers "Move unused to Recently Deleted". Nothing is deleted
  automatically. An app may delete what it made through ordinary access, which
  goes to Recently Deleted like any delete.

## Order of work

1. The derived root, its liveness, `block-location::unfiled`, and the server
   keeping it.
2. Pickers with no place, and every other creating call site, go to Unfiled.
3. The files pane section and leftovers.
4. Update guides/blocks/workspaces.md (where pickers put blocks), and add a
   decision: My files holds only what the person put there.

## Not planned here

- Sorting Unfiled into folders automatically.
