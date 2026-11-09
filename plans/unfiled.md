# Unfiled

Depends on: none.

A place for blocks made without a place the person chose, so My files only
ever holds what they put there themselves. Partly implemented; see Order of
work.

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
  a scope of its own. The server reserves the id for that account as it does
  My files (the `my_files_of` claim check in `be-server/src/organization.rs`)
  and lets it be made at the root as My files is (the `may_add_workspace`
  exception). Administrators of a company organization see it as they see My
  files.
- **Liveness.** Unfiled has no content listing its blocks: it holds every block
  whose parent names it, so making a block there edits nothing anyone else
  must be able to edit. That is a new case in `BlockGraph::holds`, which today
  treats only `BlockParent::Root` that way and otherwise needs the parent's
  references to list the child: a parent that is a reserved Unfiled id holds
  its children by parent alone. The graph learns which ids are Unfiled from
  the server (a flag on the block row set when the reserved id is made), since
  it can't derive them. Amend
  decisions/block_reclamation_by_reference_counting.md ("every parent still
  listing its child") to name the exception.
- **Taking out and deleting.** Taking a block out is a move. Deleting one is
  `SetParent(Detached)` with `DELETED_FROM` naming Unfiled, so Restore puts it
  back, since there is no content to drop it from.
- **Placing.** `block-location` gains `unfiled`, which the host resolves to the
  account's own Unfiled; apps never learn its id. Everything made without a
  place the person chose goes there: a pick with no place, a terminal an app
  makes for a connection, an exported dynamic artifact, a recording, a file
  pasted or dropped where nothing takes it.
- **The files pane.** A section below My files: a flat list, newest first.
  Each row names the app that made the block (a new defined metadata key,
  `CREATED_BY`, the app's manifest id, set by the host when an app creates a
  block) and where it is used, from its
  backrefs ("boat.dev · used in Dev VMs"). No folders are made or arranged
  there; the one thing to do is move a block out, which files it. References
  are by id, so filing breaks nothing.
- **Leftovers.** A block nothing references shows "Not used anywhere", and the
  section offers "Move unused to Recently Deleted". Nothing is deleted
  automatically. An app may delete what it made through ordinary access, which
  goes to Recently Deleted like any delete.

## Order of work

Done: the derived root and its reserved id, the server keeping it, its
liveness rule in `holds`, delete and restore, `block-location::unfiled`, the
guides and decisions. Pickers with no place, editors with no block of their
own, "New file here" beside a block nothing holds, and new database views go
to Unfiled.

1. Every other creating call site goes to Unfiled where the person chose no
   place: a terminal an app makes for a connection, an exported dynamic
   artifact, a recording, a file pasted or dropped where nothing takes it. The
   PR lists every site that creates a block and which it changed, with a test
   per changed site.
2. The files pane section (a flat list, newest first, `CREATED_BY` and where
   each is used) and leftovers, with paintings. Until then a block in Unfiled
   is not shown anywhere in the files pane.

## Not planned here

- Sorting Unfiled into folders automatically.
