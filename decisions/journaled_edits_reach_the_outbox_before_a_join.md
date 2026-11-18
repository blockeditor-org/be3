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
- **An edit made before a reload of a merge.** Merges rebuilt texts, rich texts and series from
  their items, and a fast-forward reloaded the head as loaded, so everything in the reload was
  new and an edit made before it and applied after (one on its way, a peer's, one the reload
  applies again, an editor's not yet shown the reload) named nothing and was lost, with or
  without a join. Now a merge splices ours where it differs, a fast-forward goes through one,
  and the reload carries the merge's session state, so those edits land
  (guides/blocks/content_types.md, guides/type_modules.md). An editor's edit made before a
  reload that still gives new identities (a restart) is carried over it
  (`carried_over_a_reload` in be-sdk-type-guest's replica.rs). Carrying the replica's unsent
  edits over a reload (`carry_unsent`) was dropped: every reload but a restart now keeps what
  they name, and a restart carries them already (`carry_pending`).

Edits of another build are lost by design: a journal of another build's tag is opened with none
of its entries (`replayable`), and the replica says its edits were lost (`lost_edits`). That
covers a module that comes back as a new build after a crash, and one restarted as a new build
while open, whose made-up session still speaks the old build, so the replica diverges.

Tests in be-block-type-host:
`edits_typed_offline_into_a_session_of_another_build_are_sealed_before_the_replica_diverges`,
`edits_typed_offline_while_a_reload_of_the_replicas_own_is_on_its_way_reach_the_server`,
`edits_typed_offline_whose_seal_before_joining_fails_reach_the_server`,
`an_edit_typed_while_a_reload_of_the_replicas_own_is_on_its_way_reaches_the_server_once` and
`an_edit_and_a_definition_kept_beside_another_devices_head_both_reach_the_server` (a kept seal
merged with another device's head); in be-sdk-type-guest,
`edits_on_their_way_when_another_head_lands_land_on_it` and
`an_editors_edit_made_before_two_restarts_it_was_not_shown_lands`; in be-block-model,
`an_edit_made_on_our_text_before_a_clean_merge_lands_on_the_merge` and its rich-text and series
twins.

## Rejected

- Keeping the edits as a commit of the device's own at close, or marking them in the device
  journal to give again: no replica closes holding them once its join waits for the seal.
