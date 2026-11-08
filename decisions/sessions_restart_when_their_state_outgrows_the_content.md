# Sessions restart when their state outgrows the content, and undo stops there

A live session keeps session state (tombstones, text positions, split fragments)
so concurrent edits land where their authors meant. It only grows, so a block
someone always has open would keep every tombstone forever. The owner restarts
the session at a commit once that state passes `RESTART_STATE_BYTES` (1 MiB)
and outgrows the content it describes, while no gesture is open and it is alone
in the session, or with others once the state passes `RESTART_CEILING` (4)
times the limit (`be-sdk-type-guest/src/replica.rs`, `restart_due`). It commits, tells the
followers with `SessionMessage::Sealed { restart: true }`, and every replica
reloads the commit; loading gives every replica the same positions, so they go
on exchanging operations. The last participant to leave restarts for free: it
rebases the journal with no state, so the next session loads the commit
afresh.

What each replica had pending moves onto the reloaded content
(`LiveEdit::carry_over`, `be-block-model/src/carry.rs`): a document's text and
series operations, of every kind, travel as the visible splices they made (what
they showed is inserted again by the carrying client), a mark or unmark as the
formats it leaves, and objects keep their ids. Presence that names visible
offsets (carets, selections) carries over untouched. Undo cannot: every step
names positions that no longer exist, so `History::restart` drops them, and an
undo with nothing left since the restart does nothing and the frame says
"Can't undo past here" with a button that opens version history at the
restart's commit (`block-history.restarted-at`, `block-command::history`).

## Why

- Cost, not a timer, triggers it: most sessions never restart, and one that
  restarts in the middle of editing is a sign the limit is too low.
- Reloading from the commit needs nothing new from loading, and the commit is a
  restore point that holds everything before the restart, which is where undo
  sends the user instead.
- Carrying pending edits as visible splices loses only `Delete`'s guarantee for
  an insert concurrent with someone deleting around it, and only for edits in
  flight at the restart. Waiting for the owner to be alone makes even that rare.

## Accepted limits

- An edit an editor sent its own replica, which the replica takes only after
  it reloaded, is applied as made; positions it names no longer exist, so the
  sequence refuses it and that keystroke is lost (the editor's view resyncs).
  Closing it would need the editor to acknowledge the restart's snapshot so the
  replica could carry what came before; it is a few milliseconds of one device's
  input, at a moment the owner chose because the session was quiet.

## Edits another build wrote

A journal is its module tag's (guides/blocks/client.md, "The journal"). A
replica of another build cannot replay one, so it opens on the head and its
first append or commit starts the journal over. The server marks the journal
`lost` when a rebase of another tag drops entries, and keeps the mark until the
journal moves to another commit, so every device that opens the block meanwhile
is told (`opening.lost-edits`, `block-history.lost-edits`) and its editor frame
says edits made with an older version were lost. After that commit the head
holds everything since, and there is nothing left to say.

## Rejected

- Restarting on a timer: it would stop undo for no reason most of the time.
- Mapping undo steps onto the reloaded content: a step can hold any change of
  the document, and a wrong mapping undoes the wrong thing; history keeps the
  older states already.
- Keeping session state across sessions: a journal base would carry it, and
  nothing would ever drop a tombstone.
