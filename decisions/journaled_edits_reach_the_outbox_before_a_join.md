# Journaled edits reach the outbox before a join

Decided: a replica's journaled edits that no seal holds are kept by sealing them in its made-up
session before it joins, and by holding the join until that seal lands. Nothing keeps them
after a replica closes: a plan to keep them as a commit of their own after a diverged replica
closed, give them again on opening and record a conflict where they no longer apply, was
dropped.

## Why

Reconnecting asks every replica holding unsealed work to seal in its made-up session first
(`Replicas::reconnect`), and the join's answer waits while that seal is on its way, and after it
while journaled edits are left that no seal would take now (`Replicas::settling`,
`rejoin_held`). The seal goes to the outbox as a commit the server lacks; a drain sends it and
`merge_diverged` merges it with whatever the server holds. Opening on the server's journal
instead would drop the edits: it holds none of the made-up session's, and this device's journal
is passed over for it. So each way the edits seemed to be lost reaches a commit:

- **Divergence.** A made-up session speaks the replica's own build, so a replica diverges only
  after it joins a session of another build, by which time the seal holds its edits; and a
  diverged replica refuses edits (`edit` and `hold` in be-sdk-type-guest's replica.rs), so it
  journals none after.
- **A reload of its own on its way.** The seal seals nothing while it waits, but the merge the
  reload is made from holds the edits, and the replica reopens on it.
- **A newer head on the server.** The made-up session's seal does not merge online: it is kept,
  and `merge_diverged` merges it after the drain.
- **A seal that fails** (a store write refused, a missing key, a part outside the replica's
  reach). The replica backs off and owes no seal; the join waits (`sealed_before_joining`) until
  its retry lands. Before this, the join opened the replica on the server's journal and dropped
  the edits.
- **A merge that cannot read its parts.** It stops only the due timer (`commit_at`): the seal
  before joining still goes to the outbox, and a held join waits as for a failed seal. No test
  holds this one; it follows from the code.

Edits of another build are lost by design: a journal of another build's tag is opened with none
of its entries (`replayable`), and the replica says its edits were lost (`lost_edits`). That
covers a module that comes back as a new build after a crash, and one restarted as a new build
while open, whose made-up session still speaks the old build, so the replica diverges.

Tests in be-block-type-host:
`edits_typed_offline_into_a_session_of_another_build_are_sealed_before_the_replica_diverges`,
`edits_typed_offline_while_a_reload_of_the_replicas_own_is_on_its_way_reach_the_server`,
`edits_typed_offline_whose_seal_before_joining_fails_reach_the_server` and
`an_edit_and_a_definition_kept_beside_another_devices_head_both_reach_the_server` (a kept seal
merged with another device's head).

## Rejected

- Keeping the edits as a commit of the device's own at close, or marking them in the device
  journal to give again: no replica closes holding them once its join waits for the seal.
