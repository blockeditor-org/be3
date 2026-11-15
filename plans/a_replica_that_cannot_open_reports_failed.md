# A replica that cannot open reports it failed

Depends on: none.

**Today.** `LiveContent::ready()` (`crates/be-sdk/src/content.rs`, around line 107) returns only
`Ok(())` or `Err(Unready::Loading)`: content the editor asked for is loading until its replica has
sent it, and nothing ever says it will not. A block this viewer can see whose replica never opens
(its stored content does not decode in this build, its type module fails, its parts cannot be
read) therefore stays `Loading` for good, and so does every `ContentProjection` of it.

Three waits hold a `Loaded` boundary on such a projection and so show "Loading" for ever:

- `Editor::content_of` (`crates/be-sdk-beui/src/editor.rs`, `wait_while_viewable`), since #965:
  it waits while the block is listed as viewable and its projection is not ready.
- `Editor::block_content` (`projection`), for the editor's own block.
- The view-state wait every `editor.view_state::<K>()` handle registers (the same
  `wait_while_viewable`), so text, the file tree, boat and the desktop calendar never show over a
  view block left in a form this build cannot read.

`wait_while_viewable` already lets a `Failed` projection go (only `Loading` is waited on), and
`ViewStateHandle::get` already reports `Failed` so callers start afresh; neither ever sees one.

**Steps.**

1. Carry a replica's failure to open to the editor: where the host or the type guest gives up on
   opening a block's replica for an editor (`watch_content`), send that over the app protocol, and
   have `LiveContent` hold it so `ready()`, `read()` and `revision()` return
   `Err(Unready::Failed(reason))` until the content arrives. Keep the protocol free of beui types
   (the reason is a string).
2. Make `ContentProjection::ready` and `read` pass it on as `Pending::Failed` (they map `Unready`
   through `ready()` in `crates/be-sdk-beui/src/content.rs`), waking readers when it changes.
3. Let be-sdk-test's `ContentStore` hold content that cannot be read, so its replica fails to open
   as the host's would (today a replica that cannot open in the test store just never answers).
4. Tests: a file tree whose view block holds malformed content still shows its files and starts
   its view state afresh; an editor whose own block cannot be read shows its failure rather than
   loading; `content_of` of an unreadable block lets its boundary reveal.
5. Optionally, fold in giving each block one projection: `Editor::projection_of` keys the editor's
   own block as `None` for `block_content` but as `Some(id)` when reached through `content_of` or
   the view-state handle, so the same block can have two projections. Map `Some(own id)` to the
   `None` key, and check the shells' layout restore (the workspace and desktop reopen tests),
   which broke once when the two were mixed.

**Done when** no boundary waits for ever on a block whose content cannot be read, and an
unreadable view block reads as `Failed`, so its editor starts its view state afresh.
