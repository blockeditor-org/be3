# A child change reports what the container's type did

Depends on: none.

Unlink, a move into or out of a folder, and a reorder all change a container's content through
`be_host_stack::change_child` (`AppEditor::{add,delete,replace,reorder}_child` in
`crates/be-host/src/editors/app.rs`). Each is told `Some(true)` as soon as the command is sent:
the worker hands `Command::ChangeChild` to `Replicas::change_later`
(`crates/be-host-stack/src/worker.rs`), which does not wait. The type's own answer,
`Answer::Changed(false)` from `child_operations` returning `None`, never comes back. So be-host
carries on as if the change was made: the Unlink copy flow (`process_pending_copies` in
`crates/be-host/src/main.rs`) sets the copy's parent to the container even when the container
took nothing, and a move or reorder its type refused looks done until the listing catches up.

`process_pending_copies` already asks `AppEditor::takes_replace` before duplicating, so a type
whose manifest lacks `children.replace` is refused with a notice and leaves no copy. What is left
is a refusal from inside a type that does take `Replace` (a text that names no such block any
more), and a container that goes out of reach after the copy was made.

## 1. The answer comes back

1. Give `Command::ChangeChild` a reply, and have the worker run `Replicas::change(ctx, block,
   change).await` (which already waits for the replica's `Changed`) without holding up its other
   commands: the reply resolves once the replica answers.
2. `change_child` hands be-host something it can poll on its frame (as `pending_transfers` and
   `pending_reorders` are polled today): pending, made, or refused. Use it from add, delete,
   replace and reorder alike, so there is one path.
3. Unlink gets a `CopyStage::Replacing { copy_id, block_type, answer }` that sets the copy's
   parent only once the type made the change. On a refusal, it reports with
   `notices::refuse` as `refuse_copy` does, and deletes the copy it made, which nothing lists.
   This also covers a container that goes out of reach between duplicating and replacing,
   where today the copy is left behind.
4. A move or reorder the type refused puts the block back where the file tree shows it and
   reports why, as `refused_offline` does for a container this device does not hold.

Test each in be-host's own tests with a container type that refuses at the type, not in its
manifest.

## 2. Drive can name a block inside text, and the host test it enables

A host test cannot put a reference inside a text block today. Text takes no `Add`, so the only
way in is pasting the block's URL, and a test has no way to learn a block's id: drive shows none,
and the menus have no "Copy link".

1. Add a way for drive to get a block's URL, for example a "Copy link" item in the block menu
   (`crates/be-sdk-shell/src/menu.rs`, also useful to a person) that drive reads with
   `clipboard`, or a `drive block TARGET` command that prints the id of a row or tab.
2. Add a be-host-tests case, with its apps named in `crates/be-host-tests/BUCK`, that pastes
   one block's URL into a text block in another folder and unlinks that reference twice: once
   with the text closed, once with it open. Each time, check that the text now names the copy
   and that the copy is listed under the text.
