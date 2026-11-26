# A restart carries what was numbered after its seal, on every replica

A restart (decisions/sessions_restart_when_their_state_outgrows_the_content.md) is due at a
seal, and the holder submits it right then, whatever the order numbered after that seal.
The `Reload` names the seal's commit, and its base is the seal's `through` (T). It carries the
session state as of the seal (`sealed_state`), as a merge's reload carries its own. Every
replica applies it at S in the same way (`carried_from_seal` in be-sdk-type-guest's replica.rs):

1. Build what the entries after T were made on: the commit loaded, with that state adopted.
2. Load the commit afresh.
3. Carry each entry it logged in (T, S), in order, from the first content onto the second
   (`LiveEdit::carry_over`, each under a client of its own, `CARRIED_CLIENTS` with its sequence).
4. Go on from the result, and carry what it has not had numbered on top, as before
   (`carry_pending`).

Every replica holds the same entries and the same content at S, so all of them reach the same
content. No commit holds that content until the next seal, so the journal is not rebased at
the restart. A replica opening on the journal replays the restart entry in the same way,
whatever seal the journal starts from (a server that lost a seal's rebase keeps it on an
earlier one): it reads the commit the entry names before it replays (`journal_restart_commits`),
and builds what the entries after T were made on from that commit and the state the entry
carries (`replay_restart`).

## Why

- A restart that waits for a seal holding everything never comes while others write (an agent
  streaming, two devices typing). The state meanwhile grows past what a seal's journal base or
  a submission may carry, and the block wedges.
- Carrying numbered entries is the same carry `carry_pending` already does for entries nobody
  has numbered. It only loses `Delete`'s guarantee for an insert concurrent with a delete around
  it, and only for the entries in (T, S).

## Rejected

- Holding a restart until a seal holds everything, and holding the holder's own edits back
  until one does: others' writing keeps every seal short.
- Giving up on a restart after a number of short seals: it adds a mode, and the state still
  grows past what a seal may carry.
- Restarting at S on the content as it is: no commit holds it, so a replica opening later
  could not load what the restart reloaded.
