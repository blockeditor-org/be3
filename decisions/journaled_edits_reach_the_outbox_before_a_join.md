# Journaled edits reach the outbox before a join

Decided: nothing more keeps a replica's journaled edits that no seal holds as it closes, because
none are lost. A plan to keep them as a commit of their own after a diverged replica closed,
give them again on opening and record a conflict where they no longer apply, was dropped.

## Why

Reconnecting asks every replica holding unsealed work to seal in its made-up session first
(`Replicas::reconnect`), and the join's answer waits for that seal (`Replicas::settling`). The
seal goes to the outbox as a commit the server lacks; a drain sends it and `merge_diverged`
merges it with whatever the server holds. So each way the edits seemed to be lost reaches a
commit instead:

- **Divergence.** A made-up session speaks the replica's own build, so a replica diverges only
  after it joins a session of another build, by which time the seal holds its edits; and a
  diverged replica refuses edits (`edit` and `hold` in be-sdk-type-guest's replica.rs), so it
  journals none after.
- **A reload of its own on its way.** The seal seals nothing while it waits, but the merge the
  reload is made from holds the edits, and the replica reopens on it.
- **A newer head on the server.** The made-up session's seal does not merge online: it is kept,
  and `merge_diverged` merges it after the drain.
- **A seal that cannot merge.** A replica's merge fails only when it cannot read the parts it
  needs, and that stops only the due timer (`commit_at`): the seal before joining still goes to
  the outbox. No test holds this one; it follows from the code.

Tests in be-block-type-host hold the first three:
`edits_typed_offline_into_a_session_of_another_build_are_sealed_before_the_replica_diverges`
(divergence, and the seal in the outbox),
`edits_typed_offline_while_a_reload_of_the_replicas_own_is_on_its_way_reach_the_server` (the
reload) and `an_edit_and_a_definition_kept_beside_another_devices_head_both_reach_the_server`
(a kept seal merged with another device's head).

## Rejected

- Keeping the edits as a commit of the device's own at close, or marking them in the device
  journal to give again: it guards a state no replica reaches.
