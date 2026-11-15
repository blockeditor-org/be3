# A replica that cannot open reports it failed

Depends on: #686 (land/686), whose rewrite of the replica code (`crates/be-sdk-type-guest/src/replica.rs`,
`crates/be-block-type-host/src/replicas.rs`, `crates/be-sdk-test/src/store.rs`) steps 2 and 3 build on.

**Today.** The editor's side is in place: `ToEditor::Failed(reason)` tells an editor its replica
could not open the block, `EditorMirror` holds it (and fails a snapshot that does not decode in
this build itself), and `LiveContent::ready()`, `read()` and `revision()` return
`Unready::Failed(reason)` until a snapshot that opens arrives. be-sdk-beui's projections pass it on
as `Pending::Failed`, so it reaches the nearest `Errored`, or the fallback of a `Loaded` that has
not revealed (guides/reactive.md, "Loading"), instead of holding it for ever.

Nothing sends `ToEditor::Failed` yet, so a block whose replica never opens still reads as loading
for good, and holds the `Loaded` an editor asked for its content under (`Editor::block_content`)
and every `Loaded` a read of it is made under. Where a replica gives up today:

- Its content does not load (its parts cannot be read, it does not decode in this build): the
  guest replica sets `unreadable` (replica.rs, `Then::Load`) and reports it in `status.unreadable`
  (`types.wit`, `record status`) with every status; jobs wait in `job_while_unreadable`.
- No module serves the type: `Replicas::start` returns `None` and nothing opens.
- Opening fails otherwise (joining the session, a client error): `Replicas::open` records the error
  (`H::record`) and returns.
- The block's key is missing: the replica is locked (`H::set_locked`) and reopens once the key
  arrives. That has its own state and stays as it is.

Edits set aside because they cannot be read (`refs::SetAside`) are not a failure to open: the block
opens without them.

**Steps.**

1. Send `ToEditor::Failed(reason)` to a replica's editors while it is `unreadable`, and to the
   editors that asked for a block's content when `Replicas::start` or `Replicas::open` gives up,
   with why. Keep the reason a string.
2. Let be-sdk-test's `ContentStore` hold content that cannot be read, so its replica fails to open
   as the host's would (today a replica that cannot open in the test store never answers).
3. Tests: a file tree whose view block holds malformed content still shows its files and starts
   its view state afresh; an editor whose own block cannot be read shows its failure rather than
   loading; `content_of` of an unreadable block reaches the `Errored` around its read.

**Done when** no boundary waits for ever on a block whose content cannot be read, and an
unreadable view block reads as `Failed`, so its editor starts its view state afresh.
