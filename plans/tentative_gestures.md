# Tentative gestures: edits held locally until they commit

Depends on: none.

Builds decisions/tentative_edits_wait_for_release.md. A tentative gesture's edits apply to the
local replica at once, so every view on the device previews them from the block, but are not sent
to peers or saved until the gesture commits. A gesture is tentative when the person would not
expect a half-finished one to be kept (the decision's criterion): drags (plans/declarative_dragging.md),
scrubbing, resizing, rotating, dragging through a colour area. Typing and strokes keep writing as
they happen.

## Today

- `be_block_content::history::History` keeps one `gesture: Option<Gesture>` per replica, tagged
  with the editor (`origin`) that opened it, and `begin` closes (commits) whatever gesture is
  open, even another editor's. Two windows dragging in one block commit the first gesture early;
  one editor beginning twice (a second finger, a second controller) does too, and a cancel meant
  for the first then cancels the second. `undo`, `redo` and `restart` also `close()` an open
  gesture, committing it.
- An open gesture only groups undo. Each edit goes through `Replica::edit` (be-sdk-type-guest
  `replica.rs`) at once: applied to `visible`, recorded with `journal(Journaled::Edited)` and
  `lacking.add`, then sequenced by an owner (or pushed onto its `held` FIFO while a seal is out
  or a part is being put, and later sequenced by `sequence_held`), or by a follower submitted, or
  queued on `unsent`, where `C::absorb_operation` folds it into the last queued edit, and sent by
  `flush`.
- `rebuild_visible` rebuilds `visible` as `confirmed`, then a follower's pending submissions,
  then `unsent`, then `held`.
- `be-host-runtime` (`instances.rs`) tracks one `gesturing` flag per link and cancels it when the
  instance closes.

## Shape

- **Kinds.** `Gesture::Begin` stays an undo group, written as it happens (the canvas holding a
  gesture open while new text is typed must survive a crash). `Gesture::BeginTentative` opens a
  tentative one. Commit and cancel are shared, and name what they end.
- **Ids.** A gesture is named by `(editor, id)`, not by its editor alone, so one editor can hold
  several drags and each ends on its own. `History` keeps undo groups and tentative gestures side
  by side, each under its id.
- **The tentative layer.** The replica gains `tentative: Vec<(GestureId, C::Op)>`, a layer of its
  own beside `held` and `unsent`, holding every open tentative gesture's edits in the order they
  were made: one group for the block, however many gestures are open. An edit made in a tentative
  gesture is applied to `visible` and pushed there, and nothing else: it is not passed to
  `journal` or `lacking.add`, and it never enters `held` or `unsent`. So `sequence_held` and
  `flush` never see it, and `absorb_operation` never folds a later, ordinary edit into it or it
  into one. Consecutive edits of one gesture to the same item may be folded with each other, since
  only the last position matters.
- **Applied last.** `rebuild_visible` applies `tentative` after `held`. An ordinary edit made
  while the layer is held (typing, an undo group, a peer's edit) bypasses it and takes its usual
  path at once; where it touches what a held edit touches, the held one shows, and wins once it is
  sent after it.
- **Commit.** When the last gesture in the layer commits, its edits go, in the order they were
  made, through `edit`'s usual path (`journal`, `lacking.add`, then sequenced, `held` or
  `unsent`), without being applied to `visible` again, and `History` makes them one undo step.
  This is the decision's invariant: no tentative edit goes ahead of one it was built on, while
  ordinary edits may have gone ahead of all of them.
- **Cancel.** Cancelling one gesture removes its edits from the layer and rebuilds `visible`; the
  others stay held.
- **Undo and redo** while the layer is held cancel the whole layer first, as Escape would, and that
  is the whole step: the undo stack is left alone. Redo is refused (`Changed(false)`) while the
  layer is held. Undo groups keep today's behaviour.
- **Ending without a commit.** The instance closing, the replica restarting from scratch or a
  crash drops the layer: never recorded, so nothing is replayed.
- **Peers** see a tentative edit through presence only (the drag board publishes its frames).
  Shipping the held edits in presence, so peers' derived views follow too, is left for later.

### Session events while the layer is held

What the replica does with `tentative` when its `confirmed` or role changes underneath it, and how
the editor and the drag board hear. "Carried" means each held edit goes through `C::carry_over`
from the old `confirmed` to the new, as `carry_pending` and `restarted` do for `unsent` and
`held`; an edit that does not carry (its item is gone) cancels its gesture.

| Event | The layer | Editor and drag board hear |
| --- | --- | --- |
| A snapshot, or a seal reloaded on catch-up or `reload` (`Then::Snapshot`, `Then::SealedReload`) | carried, reapplied on top by `rebuild_visible` | the usual new snapshot or projection; a gesture whose edits did not carry is ended with `GestureEnded { id, cancelled: true }` |
| Ownership change (owner to follower or back) | untouched: it was never sequenced or submitted, so moving `held` and `unsent` across roles skips it | nothing |
| A conflict stash (`stash`, `AfterStash::Keep`/`Merge`) | kept held; what is stashed is written from `visible` without the layer, so a held position never reaches a stash | nothing |
| `history.restart()` (`restarted`, or `Then::SealedReload` with `restart`) | carried; its gestures stay open, and `History::restart` keeps their steps, so a commit is still one undo step | as for a snapshot |
| `history.clear()` (a replacement: `Then::Replacement`, `FromEditor::Replace`, `replaced`) | cancelled whole: the content it moved is gone | `GestureEnded { cancelled: true }` for each open gesture |

`GestureEnded` is a new message to the editor that opened the gesture, through
`ContentProjection` (be-sdk-beui); the drag board cancels a carry whose gesture ended, so the item
glides home and the site stops previewing.

## Steps

1. `History` in be-block-content: gestures by `(editor, id)`; undo groups and tentative gestures
   side by side; `undo`, `redo` and `restart` as above. Fixes the early commit and the wrong
   cancel. Unit tests.
2. The replica (`be-sdk-type-guest`): the `tentative` layer, its commit and cancel, and each
   session event in the table. `be-host-runtime` tracks open gestures per link and cancels them all
   when the instance closes. Tests: a crash with a held drag saves nothing; two drags in one block
   commit as one step when the second ends; cancelling one of two keeps the other; a typed edit
   during a held drag reaches peers at once; a catch-up mid-drag keeps the drag; a replacement
   mid-drag ends it; undo mid-drag cancels it. Audit that each content type's operations name
   items by id, not position, and fix one that does not.
3. The app protocol and `ContentProjection` (`be-sdk`, `be-sdk-beui`): `begin_tentative`, gesture
   ids on every gesture message, `GestureEnded`. Update guides/blocks/host.md (Undo) and
   guides/adding_an_app.md (Gestures).
4. Move today's choosing gestures off writing as they go, each judged by the decision's criterion:
   - the infinite canvas's move, rotate and resize gestures;
   - `NumberInput`'s scrub, which today writes `value` every frame between `on_scrub(true)` and
     `on_scrub(false)` (the canvas sidebar's fields: `sidebar.rs` `on_scrub` →
     `CanvasState::scrub` → `open_writes`/`commit_writes`, an undo group): a half-scrubbed number
     is not one the person chose, so the scrub becomes a tentative gesture;
   - the colour picker's drag, which today previews outside the block through `on_preview` and
     writes once on release, so other views of the block do not follow it: it becomes a tentative
     gesture, and `on_preview` goes;
   - the rest found by `rg begin_gesture`, each kept as an undo group only if a half-finished one
     is something the person would expect to keep (a stroke, the canvas's new text).
