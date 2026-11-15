# A child change reports what the container's type did

Depends on: #686 (session catch-ups replaced by server ordering), which rewrites what joining does with an offline replica's edits.

A child change (`be_host_stack::change_child`) answers with what the container's type did
(`ChildChanged`): Unlink places its copy, and a move or reorder counts as made, only once the type
made the change, and a refusal puts the block back and says why (be-host's `refuse_transfer`,
`refuse_reorder` and `refuse_copy`); an unlink asks before it makes its copy, and
be-host-tests' `unlinking_a_reference_in_a_text_puts_a_copy_in_its_place` unlinks a reference in a
text, open and closed. What is left:

## 1. An offline text refuses a replaced child

A local account's replicas, which never join a session, edit as a fresh client of their own
each time they open (`unjoined_client`), so this is a server account's gap only. A text made
on this device and not yet on the server refuses every child change it would take once
synced: an offline replica edits as `OFFLINE_CLIENT` (0,
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
