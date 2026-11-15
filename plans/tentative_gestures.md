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
  `replica.rs`) at once: applied to `visible`, told to the other attached editors with
  `journal(Journaled::Edited)` (despite its name, `journal` feeds the editors' projections,
  `landed`), kept with `lacking.add`, then queued on `unsent`, where `C::absorb_operation` folds it
  into the last queued edit, and submitted to the server's order by `flush`, one in flight
  (`pending`) at a time. What survives the host crashing is what the server numbered into the
  block's journal (or, offline, what the host numbered into the device's). While the host stays
  alive, a module restart or a reopen is also given again the local edits the host keeps in
  memory (`Open::replay`, a `Vec<Replayed>` in be-block-type-host `replicas.rs`;
  guides/blocks/host.md, Opening again), cut by the counts `reported_clean(taken)`,
  `sealed-taken`, `journaled-taken`, `untaken` and `acknowledged`, and a replayed edit the order
  already holds is skipped by its number (`taken-before`, `own_through`). Those cuts and numbers
  assume that the edits submitted are a prefix of the edits taken, in the order they arrived.
- `rebuild_visible` rebuilds `visible` as `confirmed`, then `pending`, then `unsent`. A peer's
  entry the order numbers before this replica's own goes beneath them (`apply_beneath`, told to
  editors as `Journaled::Beneath`); with nothing unconfirmed it applies on top.
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
  own after `pending` and `unsent`, holding every open tentative gesture's edits in the order they
  were made: one group for the block, however many gestures are open. An edit made in a tentative
  gesture is applied to `visible`, told to the other editors (`journal(Journaled::Edited)`, so
  every view on the device previews it) and pushed onto the layer. It is not kept with
  `lacking.add` and never enters `unsent`, so `flush` never submits it and it never reaches a
  journal; `absorb_operation` never folds an ordinary
  edit into it or it into one. Consecutive edits of one gesture to the same item may be folded
  with each other, since only the last position matters.
- **Counting and replay.** A host crash loses the layer with nothing more to do, since replay
  lives in memory. A module restart or a reopen while the host is alive keeps an open gesture's
  messages (its begin, its operations, a cancel) in replay, so the new replica rebuilds the layer
  and the drag carries on. Because the cuts assume a prefix in arrival order, a tentative edit is
  not a local edit (`Replayed::local_edit`) for `sent`, `taken`, `sealed-taken`,
  `journaled-taken` or what was forwarded until its gesture commits; from then on its edits count
  in commit order, after every edit taken before the commit. The exact accounting is settled in
  step 2.
  After a commit the gesture's original messages still sit in replay before the ordinary edits
  taken during the hold, so a positional `reported_clean` cut could drop them: the host moves a
  committed gesture's messages to the commit's place in replay, or records the commit as one
  message that carries them.
- **The invariant.** At all times `visible` = `confirmed` + `pending` + `unsent` + `tentative`,
  in that order. While `tentative` is non-empty, each path keeps it by applying beneath it
  (`C::apply_beneath`, telling editors through `Journaled::Beneath`) or by rebuilding
  (`rebuild_visible`, then a reprojection):
  - a peer's entry (`apply_operation`) goes beneath with `local` being `pending`, `unsent` and
    `tentative`, and takes its fast path (apply on top) only when all three are empty;
  - `edit`, for an ordinary edit, queues it on `unsent` and rebuilds, so it lands beneath the layer;
  - an entry of the replica's own leaves `visible` as it is, since it already sits beneath the
    layer.
- **History beneath the layer.** An ordinary edit made during a hold records its undo step
  (`History::record`, `Undo::step`) against the content it is applied to, `visible` without the
  layer, which the replica keeps beside `visible` while the layer is non-empty.
- **Commit.** When the last gesture in the layer commits, its edits move, in the order they were
  made, to the end of the ordinary path (`lacking.add`, then `unsent`), and
  `History` makes them one undo step. They are already on top of `visible` and stay there, so they
  are not applied again. This is the decision's invariant: no tentative edit goes ahead of one it
  was built on, while ordinary edits may have gone ahead of all of them.
- **Cancel.** Cancelling one gesture removes its edits from the layer and rebuilds `visible`; the
  others stay held.
- **Undo and redo** while the layer is held cancel the whole layer first, as Escape would, and that
  is the whole step: the undo stack is left alone. Redo is refused (`Changed(false)`) while the
  layer is held. Undo groups keep today's behaviour.
- **Ending without a commit.** The instance closing or the host crashing drops the layer; a module
  restart rebuilds it from replay (above).
- **Peers** see a tentative edit through presence only (the drag board publishes its frames).
  Shipping the held edits in presence, so peers' derived views follow too, is left for later.

### Session events while the layer is held

What the replica does with `tentative` when its `confirmed` changes underneath it, and how
the editor and the drag board hear. `GestureEnded { id, cancelled }` is a new message to the
editor that opened the gesture, through `ContentProjection` (be-sdk-beui); the drag board cancels
a carry whose gesture ended, so the item glides home and the site stops previewing.

| Event | The layer | Editor and drag board hear |
| --- | --- | --- |
| A reload the order reaches without `restart` (a replica opening on a sealed head, `reloaded`) | reapplied as it is by `rebuild_visible`, after `pending` and `unsent`, as they are. An edit whose item is gone applies as nothing, as one in `unsent` does today; its gesture stays open | the usual reprojection; a carry whose source node was torn down ends as any such carry does |
| A restart: a reload with `restart` (`reloaded`, then `carry_pending`) | each edit goes through `C::carry_over` after `carry_pending`'s, continuing the same `before`/`after` chain rather than starting from the old `confirmed`; `History::restart` keeps the open gestures' steps, so a commit is still one undo step | the usual reprojection; a gesture with an edit that does not carry is ended with `GestureEnded { cancelled: true }` and its edits dropped from the layer |
| The lease moving to or from this replica | untouched: the lease only seals what the order holds, which the layer never reaches | nothing |
| A stash (`stash`: a conflict, `AfterStash::Merge`, or work a replacement would lose, `AfterStash::Unsaved`) | kept held; what is stashed is written from `visible` without the layer, so a held position never reaches a stash | nothing |
| `history.clear()` (a replacement: a reload with `replace`, `FromEditor::Replace`) | cancelled whole: the content it moved is gone | `GestureEnded { cancelled: true }` for each open gesture |

## Steps

1. `History` in be-block-content: gestures by `(editor, id)`; undo groups and tentative gestures
   side by side; `undo`, `redo` and `restart` as above. Fixes the early commit and the wrong
   cancel. Unit tests.
2. The replica (`be-sdk-type-guest`): the `tentative` layer, its commit and cancel, and each
   session event in the table. `be-host-runtime` tracks open gestures per link and cancels them all
   when the instance closes. Tests: a host crash with a held drag saves nothing; a module restart mid-drag keeps the drag;
   status cuts stay right with edits taken during a held drag; two drags in one block
   commit as one step when the second ends; cancelling one of two keeps the other; a typed edit
   during a held drag reaches peers at once; a peer's move of an item held mid-drag shows beneath
   the held move, and every replica ends the same after the commit; a seal mid-drag leaves the
   drag out of the commit; a restart that removes the dragged item ends it; a replacement
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
