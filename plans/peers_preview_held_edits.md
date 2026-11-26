# Peers preview held edits

Depends on: none.

A tentative edit (decisions/tentative_edits_wait_for_release.md) is applied to the holder's
replica at once, so every view on that device previews it from the block itself. Peers see it
only through presence: the canvas's carried move (`CanvasCursor::carried`, drawn by
`Entities::carry_for_peers`, with bound lines following) is the one case an app handles. Every
other held gesture (the canvas's resize, rotate, a line's end or bend, the sidebar's scrubbed
values, and every other app's drags) reaches peers only on release, so their derived views
(bound lines, z-order, a builder's layout) jump. The decision's reason for previewing from the
block applies to peers too: derived views come out right without each app learning about
previews.

The aim: a holder's replica sends its tentative layer to the session's peers as it grows, and a
peer's replica shows it as a layer of its own on top of everything it holds, which its editors
see as unsettled edits. The layer is never written by the peer: it leaves when the holder's
commit is numbered, when the holder cancels, or when the holder leaves.

## Design

- **On the wire.** A new `SessionMessage::Held` (be-block-session's `sequencer.rs`, beside
  `Presence` and `Channel`), relayed like presence: `{ layer, from, operations, commits_as }`.
  `layer` numbers the holder's layers, so a message about an earlier one is dropped. `from` is
  how many of the layer's operations the peer should already hold: the operations are appended
  after them, and a peer holding fewer asks for the whole layer again. `commits_as` is the
  holder's local edit number that the layer's first operation will go out as once it commits, or
  none while it is held. A `Held` with no operations and no `commits_as` drops the layer (a
  cancel). Encoded operations are absorbed (`LiveEdit::absorb_operation`) before sending, so a
  drag's many frames stay a few operations, and the message stays under
  `LARGEST_RELAYED_OPERATION`. A larger layer is not previewed.
- **Who may send one.** Only a client that may edit the block: a peer drops a `Held` from a
  reader, as the server refuses a reader's submissions. A late joiner is sent each held layer
  as it joins, as `show_presence_to` sends presence.
- **Where it sits on the peer.** `visible` is `confirmed`, then the peer's own unconfirmed
  edits, then its own tentative layer, then each peer layer, ordered by client id. Peer layers
  count in `unsettled()`, so their operations reach editors as `Landed::On` (a frame of the
  drag is one cheap landing). A local edit made meanwhile goes beneath every layer
  (`Landed::Under`, from plans/backlog.md §25, with `above` counting them all), and a peer's ordered entry goes beneath them
  (`Landed::Beneath` with them in `local`).
- **Leaving.** The holder's commit goes out as ordinary edits. When the peer applies an entry
  from that holder's origin whose `through` reaches `commits_as`, the layer leaves in the same
  step: it lands as `Beneath` with `local` no longer holding the layer, so no frame shows both
  the layer and the committed edit, or neither. A cancel `Held`, the holder leaving the session
  (`adopt` drops it from the clients), or a reload or replacement drops the layer. For a cancel
  this goes through the same `Beneath`-shaped landing, which needs one with no operation: a
  variant of `Landed` that only replaces `local`.
- **Never kept.** Like the peer's own tentative layer, peer layers are left out of seals,
  stashes, `content`, descriptions and `unheld`. The peer's own edits are recorded for undo
  against the content without them, and peer layers never touch the peer's `History` or `Holds`.

## Open questions

- **A local edit on a previewed item.** A peer can edit what another peer's layer is moving.
  Its edit is made on what it sees, layer included, but goes beneath the layer. Last writer
  wins by order either way. Decide whether an app should be told that an item is held elsewhere
  (presence already says so for the carry) or whether this needs nothing.
- **Ordering several peers' layers.** Ordering by client id is stable, but two layers moving
  one item show one of them. That is acceptable while presence also draws both cursors; confirm
  it.
- **Cost.** Every frame of a drag becomes a relayed message and a landing in every peer's
  editors. Presence frames already travel at that rate. Measure whether the relay should
  throttle `Held` to the frame clock.
- **The canvas's carry.** A carried move reaches peers as presence today and is not a
  tentative edit until the drop. Once held edits are previewed, decide whether the carry moves
  through the hold as it goes (then `carry_for_peers` can go) or keeps its own presence.
- **Types that rebase.** A held operation is made on the holder's content. Applied on top of
  the peer's own unconfirmed edits it rebases like any peer entry, but a text type whose
  positions shift may preview it in the wrong place until the commit lands. Decide whether
  such types opt out.

## Steps

### 1. The message and the holder's side

Add `SessionMessage::Held`, and have the holder's replica broadcast it as its layer grows (from
`hold`), on a cancel that empties it, and with `commits_as` set at `commit_layer`. Done: a
be-sdk-type-guest Sim test records what a holder sends through a held drag: one `Held` per frame,
appended, and a final one naming the commit's edit number. Another test shows a cancel sends
the drop, and a reader's `Held` is dropped by its peers.

### 2. The peer's layer

Keep each peer's layer on the replica, apply it on top of `visible`, count it in `unsettled()`,
and land its operations on editors. Add the `Landed` variant that only replaces `local`, for a
layer that leaves without an entry. Done: a Sim test shows a peer's mirror following a held
drag frame by frame with no snapshot, typing on the peer during it landing beneath, and the
layer leaving at the commit's entry (one landing, never showing both) and at a cancel. The
peer's `content`, seal and undo never hold it.

### 3. Leaving with the holder

Drop a layer when its client leaves the session, at a reload or replacement, and when a `Held`
arrives about a newer layer. Send late joiners the layers held. Done: Sim tests for a holder
that crashes mid-drag, which leaves no layer on its peer, and for a peer that joins mid-drag,
which previews the drag from its next frame.

### 4. A canvas test with a bound line

Done: a canvas test with two editors on two devices. One resizes a shape that a line is bound
to. The other's painting shows the line following each frame of the resize before release,
and the same once the resize commits.

### 5. Guides and the decision

Update decisions/tentative_edits_wait_for_release.md ("is shown to peers through presence"
becomes "previewed by peers' replicas as a layer of their own"), guides/blocks/host.md (Undo),
guides/type_modules.md (Tentative gestures, and Peers for the message) and guides/canvas.md
(Gestures). Done: the guides describe the layer as built, and the canvas section says which
held gestures peers preview.
