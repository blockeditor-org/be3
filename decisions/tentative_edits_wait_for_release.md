# Tentative edits wait for release; everything else is written as it happens

Whether an edit is written as it happens or held until release follows what the person expects
to be left with if the gesture stops halfway (a crash, the instance closing, Escape):

- **Written as it happens** when each part is already something the person made and would expect
  to keep. A partly drawn line is saved as it is drawn; text is saved as it is typed. Grouping a
  burst for undo (`begin_gesture`, `commit_gesture`, `break_step`) does not delay those writes.
- **Tentative** when the gesture is choosing one result and only the release says which. A drag
  that moves a layer halfway is clearly not a move anyone made. So are reordering, resizing,
  rotating, scrubbing a value and dragging through a colour area.

A tentative edit is applied to the local replica at once, so every view on this device previews
it from the block itself, and is previewed by peers' replicas as a layer of their own; it is sent
and saved only when it commits. Cancelling it, a crash, or the instance closing leaves no trace. A tentative edit only
moves or changes what already exists; anything a drop creates is created on commit.

Concurrent tentative edits in one block (two fingers, two controllers, two windows) join one
tentative gesture, which commits as one step when the last of them ends; cancelling one reverts
only its part. Edits in different blocks are independent. How the replica and the app protocol hold them:
guides/blocks/host.md (Undo) and guides/type_modules.md (Tentative gestures).

The invariant on order: a tentative edit is never sent ahead of a tentative edit it was built on.
Other edits (typing during a drag, a peer's) may be sent ahead of held ones. That is safe because
operations name items by id and a tentative edit creates nothing, so nothing sent can refer to
something peers have not received.

## Peers' previews

A replica relays its tentative layer as it grows (`SessionMessage::Held`; guides/type_modules.md,
Peers preview held layers), and each peer shows it on top of what it holds, never writing it:

- **Where it shows.** A peer's layer shows above the device's own unconfirmed edits and beneath
  its own tentative layer, peers' layers ordered by client id. So each device sees its own drag
  on top: two people dragging one item each see their own, and presence draws both cursors.
- **An edit to a previewed item** needs nothing of the app: it is made, sent and goes beneath
  the layer, which shows what the commit, ordered after it, will leave. Presence already says
  who holds what.
- **Leaving.** `commits_through` names the last local edit the layer goes out as, not the first:
  a long layer may go as several submissions, and the layer leaves only once the order holds
  all of it, in the landing of the entry that does, so no frame shows the edit twice or not at
  all. Until then it stays beneath the holder's next layer, should another drag start first. A
  cancel, the holder leaving or losing edit access, or a reload drops it.
- **Cost.** One `Held` a frame of a drag, appending that frame's edit: measured on the canvas
  (`a_held_resize_relays_one_small_message_a_frame`), about 70 bytes a frame against about 125
  for the same frame's presence, and one landing at each peer. Input comes a frame at a time,
  so held layers relay no faster than presence and are not throttled. Every
  `APPENDS_BETWEEN_WHOLE` frames, and for a late joiner, a peer that missed a frame or a
  cancel of one of several gestures, the layer goes whole, absorbed, which a model document
  absorbs into one write of each field, so what a peer holds of a long drag stays small; a
  peer stops showing a layer past `LARGEST_PEER_LAYER` edits or `LARGEST_RELAYED_OPERATION`
  bytes until the next.
- **The canvas's carry** goes through the hold like any other gesture, so its presence
  (`CanvasCursor::carried`, `carry_for_peers`) went: bound lines follow it from the block.
- **Types that rebase.** No type opts out. A held edit only moves or changes what exists, by id,
  so it applies beneath a peer's own edits like any peer entry; a type whose held edits were
  positional would preview them where they land on the peer until the commit, which is what
  the commit does too.

This is about block content: what a block is, which peers receive as edits. View state (a scroll
position, the caret, open folders, the shell's dock layout) is the viewer's own and is written on
every change, never held back (guides/blocks/workspaces.md, View state), so a drag whose only
result is view state, such as resizing a split in the shell's dock, writes it as it goes. A tab
dropped in the dock changes the layout only on release because the dock previews the drop from
the place the drag board chose (plans/declarative_dragging.md), not because a write is held.

## Why

A position nobody has let go of is not a decision, so a crash mid-drag must not save it, while a
half-drawn line is a line the person drew. Previewing from the block itself keeps derived views
(bound lines, z-order, a builder's layout) right without laying out states that do not exist.
Joining concurrent drags keeps the order invariant without rebasing one held gesture over
another. Saving is batched by the client (at most every 750 ms), so writing as it happens does
not mean saving often.

## Rejected

- Writing every frame of a drag: saves states nobody chose, and fills peers' histories and side
  effects with them.
- Previewing drags only outside the block, drawn by the site: every derived view would have to
  learn about previews, or preview through speculative layout.
- Showing peers held edits through each app's presence (the canvas's carried move did): each
  app redraws only what it thought to, and bound lines, z-order and layout jump at release.
- Showing peers' layers above the device's own: a person dragging an item someone else drags
  would see it jump to the other's pointer.
- Throttling held layers below the frame rate: they cost less than presence, which is not
  throttled, and a coarser preview would jump.
- Sending the layer whole every frame: a drag's edits absorb into a few, but an append is the
  frame's edit alone, and a peer that misses one asks for the whole (`HeldAgain`).
- Writing a typed or drawn gesture once, on release: a crash would lose what the person made.
- Holding view state back until release as well: it is not an edit peers receive, and a crash
  that keeps a half-dragged split costs nothing a person cannot drag back.
- Independent tentative gestures in one block: committing one while another is open would send
  edits ahead of tentative edits they were built on, unless one gesture were rebased over the
  other.
