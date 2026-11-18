# A seal's journal base state goes by part

Depends on: none.

A restart carries what the order numbered after its seal
(decisions/a_restart_carries_what_was_numbered_after_its_seal.md), so it no longer waits. With
others in the session, though, one comes due only once session state passes `RESTART_CEILING`
times `RESTART_STATE_BYTES` (4 MiB), which is what a seal's journal base or a submission may
carry (`MAX_RELAY_BYTES`). A seal past that fails, so a session whose restart is late wedges.

## How

1. **Seal states by part.** A seal's journal base state (`seals_journal`, the server's journal
   base) goes by a part past `LARGEST_RELAYED_OPERATION`, as a reload's does, so a session
   whose restart is late still seals.

## The test

- A session whose state passes what a submission may carry still seals.
