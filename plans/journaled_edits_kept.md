# Journaled edits a diverged replica leaves are kept

Depends on: none.

A replica that diverged from the order, or whose seal cannot merge, seals nothing as it closes,
so the edits it journaled and no seal holds (as a held rejoin's, `rejoin_held` in
be-block-type-host's replicas.rs) are left only in this device's journal. `ended` drops them
from its open edits as the order holds them, and a replica opened online passes that journal
over for the server's when there is one (`journal_for`), so they are lost.

Decided: they are kept. Losing typed work is the worst outcome. The next time the block opens
they are given again; where they no longer apply, they surface as a conflict, conflicts being
data (guides/blocks/version_control.md), rather than dropped.

## How

Each part is a step of its own, with its test.

1. **Close hands them to something durable.** As a diverged replica closes with journaled
   edits no seal holds, the host keeps them: as a commit of this device's own on the base the
   replica last sealed or loaded (`Open::status.head`), made from the edits the module can still
   describe (its `carry_over` of the journal's unsealed entries), pending in the outbox like any
   commit the server lacks. Where the module cannot make one (its state unreadable), the entries
   stay in the device journal, marked as owed (`kept_after_divergence`), so step 2 finds them.
2. **Opening gives them again.** A block with such a commit, or such marked entries, opens on its
   head as one with a pending commit does today (`peer.pending_commits`): the drain sends the
   commit, the server finds it diverged from its head, and `merge_diverged` merges it. Marked
   entries are given again to the replica opened online before the server's journal is applied
   (`journal_for` keeps them instead of passing the device journal over), once, by their ids, so a
   second open does not give them twice.
3. **What no longer applies is a conflict.** `merge_diverged`'s merge by parts
   (guides/type_modules.md, "Merging by parts") records what it cannot merge as a conflict
   (`ReplicaHost::conflict`, be-host-stack's `Shared::conflicts`), which the host shows as it
   shows a merge's today, with this device's copy as `ours`. Nothing is dropped silently: an edit
   that applied lands in the head, and one that did not is in the conflict's `ours`.

## The harness

No test makes a replica really diverge today. Step 1 starts with one in be-block-type-host's
tests (beside `an_edit_carried_over_a_restart_keeps_the_identity_it_goes_again_under_across_a_crash`):
a block opened online on two devices, one of which has its replica's order cut (the server
trims the journal under it to a seal it never heard, so its next `Sequenced` is past a gap it
cannot fill) while it holds journaled edits that are not sealed. Its module reports
`Status::diverged`, which the harness waits for. It controls when the replica closes and when
the block opens again, online or not.

## The test

For each step, counted exactly:

- Step 1: the diverged replica closes holding three journaled edits; the block holds a pending
  commit with exactly those three, or the device journal holds them marked.
- Step 2: the block opens online; the head the server holds afterwards has the three edits once,
  and a second close and open adds nothing.
- Step 3: the other device edited the same part meanwhile so one of the three does not merge;
  the head has the two that applied, and the block shows one conflict whose `ours` holds the third.

## Rejected

- Giving the edits up, as the replica does now: loses typed work silently.
- Giving them again on the content a reload replaces, without a conflict: an edit made against
  content that changed under it may apply somewhere it was not meant to.
