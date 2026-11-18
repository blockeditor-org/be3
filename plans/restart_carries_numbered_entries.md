# A restart carries what was numbered after its seal

Depends on: none.

A restart (be-sdk-type-guest's replica.rs, guides/type_modules.md "Restarts") reloads the
holder's last seal afresh and applies nothing on top of it, so it waits for a seal that holds
everything the order numbered, and for the holder's own edits on their way to be numbered
(`restart_short`). A holder alone gets one within a round trip. With others writing (an agent
streaming, two devices typing) every seal can fall short, and a due restart waits for good: the
session state grows past `RESTART_CEILING` times the limit (the replica reports it once), and in
the end past what a seal's journal base or a submission may carry (4 MiB, `MAX_RELAY_BYTES`;
8 MiB a frame), after which seals fail and the block wedges. This plan makes a restart carry
what the order numbered after its seal, so no restart waits.

## How

1. **The restart names its seal and carries the state at it.** The holder submits the restart
   with its base at the seal's `through` (T) and with the session state as of the seal
   (`sealed_state`) as its `Reload` state, inline or by the part it is put as (the route a
   merge's reload takes). Every replica, applying it at S, builds what the entries after T were
   made on: the seal's commit loaded and that state adopted, the content as of T under the old
   identities. It carries each entry it applied in (T, S), in order, from that content onto the
   commit loaded afresh (`LiveEdit::carry_over`, as `carry_pending` does for what is not
   numbered), and goes on from the result. Every replica holds the same entries and content at
   S, so all reach the same result.
2. **Logs reach back to the seal.** Each replica keeps the entries it applied since the last
   seal it knows of (`log`, at most `LOG_LIMIT`): a holder's since its seal, a follower's since
   it opened (on the journal from the last seal) or heard a seal. A restart whose T is older
   than what a replica's log holds cannot be carried there: the holder restarts only on a seal
   every participant has heard (`replica.sealed`), and a replica missing entries opens again on
   the server's journal rather than guess.
3. **Journal replay performs the restart.** After such a restart the content is the seal's
   commit plus what was carried, which no commit holds until the next seal, so the journal is
   not rebased at the restart and a replica opening on it replays the restart. Replaying a
   restart entry does what applying it does (load the commit, adopt its state, carry the
   entries since T), instead of only counting it (`replay_entry`), so the entries after it land
   on the identities they were made on.
4. **Seal states by part.** A seal's journal base state (`seals_journal`, the server's journal
   base) goes by a part past `LARGEST_RELAYED_OPERATION`, as a reload's does, so a session
   whose restart is late still seals.

## The test

- A peer streams edits (one per tick) while the holder's seal comes due past the restart's
  size: the restart lands at the first seal, and every edit, the peer's and the holder's, is
  in the content once.
- A replica opening on the journal between the restart and the next seal holds the same content
  as the participants.
- A session whose state passes what a submission may carry still seals.

## Rejected

- Holding a restart until a seal holds everything, for good: under others' writing it never
  comes, and the session wedges.
- Giving up on the restart after a number of short seals: it adds a mode and the state still
  grows past what a seal may carry.
