# Editability reaches editors through `be_host_stack::access`

Whether an editor may edit a block now has one answer, `be_host_stack::access(block)`: the
graph's access, capped at `View` while the block is marked because its local edits were
discarded and no join has said since that it may be edited (`Seen::marked`, which the replicas
publish through `Host::set_marked` once the replica a join opens has its editors attached as
the join says). be-host-runtime attaches editors with it, opens an app with it
(`OpenBlock::editable`) and sends `EditorEvent::SetEditable` when it changes; a stack test
without the app protocol waits for it.

## Why

- Apps already follow `editable()` from `OpenBlock` and `SetEditable`, so a block that becomes
  read-only or editable for any reason reaches every editor with no change to the apps.
- The app protocol keeps a plain bool, with nothing of beui in it.

## Rejected

- A reopened replica sending its editor the same snapshot again when only editability changed.
  It is not what a snapshot is for, it brings back the duplicate snapshots a reopen skips
  (`snapshots_shown`), and an editor learns nothing from it about whether it may edit.
- A second flag beside `access` for the discard's mark (in `ToEditor` or the app protocol):
  two values for one question, and the editors honouring one would miss the other.
