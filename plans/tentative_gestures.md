# Tentative gestures: edits held locally until they commit

Depends on: none.

Builds decisions/tentative_edits_wait_for_release.md. A tentative gesture's edits apply to the
local replica at once, so every view on the device previews them from the block, but are not sent
to peers or saved until the gesture commits. Drags (plans/declarative_dragging.md), scrubbing and
resizing use it; typing and strokes keep writing as they happen.

## Today

- `be_block::history::History` keeps one `gesture: Option<Gesture>` per replica, and `begin`
  closes (commits) whatever gesture is open, even another editor's. Two windows dragging in one
  block, or one editor beginning twice (a second finger, a second controller), commit the first
  gesture early, and a cancel meant for the first cancels the second.
- An open gesture only groups undo: its edits are sent and journaled as they are made.
- `be-host-runtime` (`instances.rs`) tracks one `gesturing` flag per link and cancels it when the
  instance closes.

## Shape

- **Kinds.** `begin_gesture()` stays an undo group, written as it happens (the canvas holding a
  gesture open while new text is typed must survive a crash). `begin_tentative()` opens a
  tentative one. The commit and cancel calls are shared, and name what they end.
- **Ids.** A gesture is named by `(editor, id)`, not by its editor alone, so one editor can hold
  several drags and each ends on its own.
- **Joining.** In one replica, every open tentative gesture joins one held group. Its edits are
  applied to the visible content and kept in history, but are neither handed to the outbox nor
  journaled. The group commits, as one undo step, when the last gesture in it commits; only then
  are its edits sent and saved, in the order they were made. Cancelling one gesture reverts its
  edits alone, against the current content (`Undo::revert`, as undo does after later edits); the
  others stay held.
- **Other edits meanwhile.** A typed edit, or a peer's, made while a group is held is applied,
  sent and saved as usual. A tentative edit only moves or changes what exists (the decision), so
  nothing sent can refer to something peers have not received. Check each content type's
  operations refer to items by id, not by position; one that does not is listed and fixed in
  step 2.
- **Ending without a commit.** The instance closing, the replica restarting or a crash drops the
  held group: cancelled in memory, never journaled.
- **Peers** see a tentative edit through presence only (the drag board publishes its frames).
  Shipping the held edits in presence, so peers' derived views follow too, is left for later.

## Steps

1. `History`: gestures by `(editor, id)`; undo groups and tentative gestures side by side; the
   held group and selective cancel. Fixes the early commit and the wrong cancel above. Unit tests
   in be-block.
2. The replica (`be-sdk-type-guest`) keeps held edits out of the outbox and the journal and
   releases them on the group's commit; `be-host-runtime` tracks open gestures per link and cancels
   them all when the instance closes. Tests: a crash with a held drag saves nothing; two drags in
   one block commit as one step when the second ends; cancelling one of two keeps the other;
   a typed edit during a held drag reaches peers at once; the content types' operations audited.
3. The app protocol and `ContentProjection` (`be-sdk`, `be-sdk-beui`): `begin_tentative`, and
   gesture ids on every gesture message. Update guides/blocks/host.md (Undo) and
   guides/adding_an_app.md (Gestures).
4. Move today's drags off writing every frame: the infinite canvas's move, rotate and resize
   gestures, then the rest found by `rg begin_gesture`.
