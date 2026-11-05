# Saving live edits less often

Built: the journal of operations, commits at pauses, pruning as commits arrive,
journals tied to the module tag, and session restarts with the undo boundary
(guides/blocks/client.md, "The journal"; guides/type_modules.md, Peers;
decisions/sessions_restart_when_their_state_outgrows_the_content.md). What is
left:

- An owner restarts at the first commit over the limit whether or not others
  are in the session; waiting for a moment it is alone, as long as the state
  stays under a hard ceiling, would carry fewer edits over.
- An edit an editor sent its replica but the replica had not taken when a
  restart reloads it is applied on the reloaded content as it was, not carried
  over; it only matters for the few milliseconds an edit spends between them.
- `LiveEdit::carry_over` drops a pending undelete, swap, move or mark across a
  restart, since they name positions only.
- The lost-edits mark lives on the device that opened the block on another
  build's journal; other devices that open it later are not told.
