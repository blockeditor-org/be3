# A child change reports what the container's type did

Depends on: none, but step 1 follows #686 (session catch-ups replaced by server ordering), which rewrites what joining does with an offline replica's edits.

A child change (`be_host_stack::change_child`) answers with what the container's type did
(`ChildChanged`): Unlink places its copy, and a move or reorder counts as made, only once the type
made the change, and a refusal puts the block back and says why (be-host's `refuse_transfer`,
`refuse_reorder` and `refuse_copy`). What is left:

## 1. An offline text refuses a replaced child

A text made on this device and not yet on the server refuses every child change it would
take once synced: an offline replica edits as `OFFLINE_CLIENT` (0,
`crates/be-block-type-host/src/replicas.rs`), which is the sequence library's `LOADED`, and
`Sequence::replace` refuses an edit made as `LOADED`. So unlinking a reference in a text made
offline, or just made, is refused. The same id makes the host's other edits through an
offline replica (`FromFile`, `carry_over`) fail on sequences. Give an offline replica an id
of its own that is not `LOADED` and that another device's offline edits do not share, and
test an unlink in a text that has not reached the server. #686 rewrites what joining does
with an offline replica's edits, so this follows it. After #686, `OFFLINE_CLIENT` is used by
`Replicas::open` (`offline_session`, and its offline branch) and by `start`'s
`before_join = joining || session.owner == Some(OFFLINE_CLIENT)`: with an id made per
offline open, that check reads `Open::made_up` instead. A replica's own submissions are told
apart by `opening.origin` (`refs::OpenEdits`), not the client id, so a new id leaves their
deduplication alone.

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
