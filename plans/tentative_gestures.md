# Tentative gestures: edits held locally until they commit

Depends on: none.

Builds decisions/tentative_edits_wait_for_release.md. A tentative gesture's edits apply to the
local replica at once, so every view on the device previews them from the block, but are not sent
to peers or saved until the gesture commits. A gesture is tentative when the person would not
expect a half-finished one to be kept (the decision's criterion): drags (plans/declarative_dragging.md),
scrubbing, resizing, rotating, dragging through a colour area. Typing and strokes keep writing as
they happen.

## Today

- `be_block_content::history::History` keeps gestures by `GestureId { editor, id }`, undo groups
  and tentative gestures side by side (step 1); the replica keeps the tentative layer (step 2);
  and the app protocol names each gesture by an id of its editor's, so an editor opens
  tentative gestures and holds several at once (step 3). No editor's gesture is tentative yet:
  today's drags, scrubs and the colour picker still write as they go (step 4).
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
- `be-host-runtime` (`instances.rs`) tracks the ids of the gestures each link holds open and
  cancels every one when the instance closes.

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
  in commit order, after every edit taken before the commit. As built (step 2):
  - `be_sdk_type_guest::holds::Holds` decides from the messages alone, on the host and in the
    replica alike, which messages are held: an editor's `BeginTentative`, its operations while
    its tentative gesture is open, and the commit of one that waits for the rest. A tentative
    gesture's begin, commit and cancel never count as local edits. Its operations count when the
    layer commits (the last open tentative gesture ends, by a commit, a cancel, a begin under its
    name or its editor detaching), every committed gesture's in the order they arrived, before
    the message that ended it. A cancelled gesture's never count, nor does anything of the layer
    an undo cancels.
  - The host keeps held messages in `replay` where they arrived, marked held
    (`Replayed::Message::held`): no `reported_clean` cut takes them, `kept_edits` does not keep
    them on the device, and a reopen after joining keeps them. When the layer commits, the
    operations that count move to the end of `replay` as ordinary messages and the gesture
    messages are dropped, so a replica given them again makes them ordinary edits, numbered as
    the first counted them; a cancel drops its gesture's messages, and an ending commit or cancel
    is sent to the module but kept nowhere. A `Change::Undo` and an editor detaching are followed
    the same way.
  - The replica numbers a committing layer's edits after the edit taken last before the message
    that ended it, in the order of the counted messages, and runs each through the checks a
    local edit given again goes through (`own_through`, stale, uncertain, doubtful) before it
    joins `unsent`.
  - A gesture the replica ends itself (a replacement, an edit that does not carry over a
    restart) stays open in `Holds` and in `History`, its edits dropped and its later ones applied
    as nothing (`voided`), until its editor ends it: the host cannot see it end, so the counts
    follow the editor's messages alone. Step 3's editor answers `GestureEnded` by cancelling.
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
- **Steps beneath the layer.** A layer edit's undo step is recorded against `visible` with the
  layer, so an ordinary edit made beneath it to the same item afterwards leaves that step's
  `before` stale: undoing the commit restores the value from before the drag, not the one the
  ordinary edit wrote. That is expected; the drag and the edit beneath it touched the same
  item, and the drag committed last.
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

1. Built. `History` in be-block-content: gestures by `(editor, id)`; undo groups and tentative gestures
   side by side; `undo`, `redo` and `restart` as above. Fixes the early commit and the wrong
   cancel. Unit tests (`history::tests`, whose `Desk` models the replica's layer). As built:
   - `GestureId { editor, id }` names a gesture and `begin(gesture, GestureKind)` opens either
     kind; `record` takes the id of the gesture an edit was made in, and an edit naming no open
     gesture is recorded as one made outside any. The replica passed one id per editor
     (`EDITOR_GESTURE`) until step 3 put ids on the protocol.
   - The layer's steps are kept in edit order with their gesture, a gesture's consecutive edits
     joined, so a cancel drops only its part; a committed gesture's part waits for the rest, and
     when the last open tentative gesture ends, by a commit or a cancel, what is left becomes
     one step. `holding()` says a layer is open, and the caller writes its layer when it stops.
     A cancel of a tentative gesture returns no operations: the caller drops them from its layer.
   - Tentative edits leave the redo stack alone until the layer commits, so an undo that
     cancels the layer leaves redo as it was.
   - `undo` and `redo` return `Stepped { operations, cancelled }`. A non-empty `cancelled` means
     the caller drops its whole layer, committed gestures' waiting edits too; it names only the
     open tentative gestures, for `GestureEnded` in step 2. `tentative()` lists the
     open ones, for a `clear()` to end.
   - `begin` only opens a gesture, and debug-asserts its name is not open. The caller commits
     a gesture open under that name first, as the replica's `Gesture::Begin` does, so a caller
     keeping a layer sees it end.
   - `can_undo` is true while a layer is held, since an undo would cancel it.
2. Built. The replica (`be-sdk-type-guest`): the `tentative` layer, its commit and cancel, and
   each session event in the table, with the accounting above. Tests in be-sdk-type-guest
   (`holds::tests`, and the replica's `tests` driving `Gesture::BeginTentative` as an editor
   would), be-block-type-host (a module restart mid-drag keeps the drag, with the status cuts
   around it; a host crash with a held drag saves nothing) and be-host-runtime (closing an
   editor drops the tentative gesture it left open). As built:
   - `Gesture::BeginTentative` and `ToEditor::GestureEnded { id, cancelled }` are on the type
     module's editor messages (step 3 has editors follow `GestureEnded`).
   - `be-host-runtime` kept one flag per link, set by either begin, and cancelled the
     gesture when the link went (step 3 made it a set of ids).
   - An editor detaching cancels its own tentative gesture in the replica and the host.
   - The layer folds nothing: each held edit is its own entry, weighing one in `unsettled`.
   - `content`, descriptions, `FromFile` and stashes read the content without the layer.
   - `Document::carry_over` drops a change naming an object the content loaded afresh lacks, so
     a restart that removed the dragged item ends the drag.
   - A committed gesture waiting for the rest is lost with the layer by a crash, as one gesture.
   - An undo while the layer is held voids the open gestures in `Holds` as well, since the host
     sees it: they stay open, what their editors send before hearing is held, and none of it
     counts however they end. Left as is: a second undo while a voided gesture is still open
     is swallowed, and redo is refused, until its editor ends it, alike on host and replica.
   - A gesture the replica voids itself (a replacement, an edit that does not carry) is voided
     only in the replica, since the host cannot see it. Its editor ending it with a commit
     counts its operations though they were dropped, and a replica opened again from `replay`
     before a status covers them makes them ordinary edits. Step 3's editor answers
     `GestureEnded` with a cancel, which leaves only a commit that crosses it.
   - Costs left as they are: an ordinary edit made while the layer is held rebuilds `visible`
     and snapshots every editor, per keystroke (`Journaled::Beneath` with the layer as `local`
     would tell them less); and since `unsettled` counts the layer, a peer's entry while it is
     held may rebuild `visible` and snapshot the editors rather than land beneath it.
   - The audit: every content type names items by id. `be_block_model` changes name
     `ObjectId`s, anchors and sequence `Pos`es; blobs, the chat, the search index and the test
     fixtures set whole values or append. The one index, the `Sections` test fixture's
     `SetSection { index }`, names one of ten fixed slots that never shift, so it was left. One
     ordering hazard is not about positions: a text or series insert must reach the order in
     the offsets its client allocated, so a tentative edit must allocate none, which a gesture
     that only moves or changes what exists (the decision) never does.
3. Built. The app protocol and `ContentProjection` (`be-sdk`, `be-sdk-beui`): `begin_tentative`,
   gesture ids on every gesture message, `GestureEnded`. Tests in be-sdk-type-guest (`holds::tests`
   and the replica's `tests`: one editor's two drags ending on their own; an editor cancelling a
   gesture a replacement ended, so a frame it sent before hearing never counts), be-sdk
   (`content::tests`: the projection answering `GestureEnded`) and be-host-runtime (a tentative
   gesture written when it commits through the instance's link; closing cancels two). As built:
   - `FromEditor::Gesture { id, gesture }` begins or ends the editor's gesture `id`, and
     `FromEditor::Operate { gesture, operation }` names the gesture an edit was made in, or none.
     `Gesture::Break` became `FromEditor::Break`, since it names no gesture. `EDITOR_GESTURE`
     is gone: `Holds`, `History` and the replica take the ids from the messages.
   - An edit names its gesture explicitly rather than joining whatever its editor holds open:
     an edit naming none is recorded outside every gesture. be-sdk's `LiveContent` (and
     `ContentProjection`) return a `ContentGesture` from `begin_gesture` and `begin_tentative`,
     take it in `operate_in`, `commit_gesture` and `cancel_gesture`, and number gestures per
     `EditorHost`, so two projections of a block in one instance never share an id. The canvas,
     the one caller, keeps its open undo group and writes through it.
   - `EditorMirror` follows the editor's open gestures. On `GestureEnded { cancelled: true }` for
     one still open it answers with a `Cancel` for it, drops what the editor makes in it after,
     and sends nothing when the editor ends it; `LiveContent::on_gesture_ended` (and
     `ContentProjection::on_gesture_ended`) tell the editor, which stops the gesture. A
     `GestureEnded` with `cancelled: false` is ignored: the replica sends none.
   - An edit the replica drops in a gesture it ended is answered with a snapshot to its editor,
     as a refused edit is: otherwise the editor's mirror would keep it as pending, and take its
     next own edit landing for that one.
   - The host's `GestureEnded` now reaches the projection through `EditorHost`'s content
     updates, like a snapshot.
   - The mirror sends an edit in a gesture only while that gesture is open: one through a
     handle kept after its gesture ended, or in a gesture begun before the content arrived or
     while the block was not editable (its begin never sent), is dropped, never written as it
     happens.
   - Left as they are:
     - A snapshot answers each edit the replica drops in a gesture it ended, which the editor
       sent before hearing. Tagging the mirror's pending edits with their gesture would let it
       drop them itself on `GestureEnded`; step 4 can do that if drags send enough frames in
       flight for it to matter.
     - `EditorHost::content_of` makes a new `LiveContent` per call, and two live at once for one
       block in one instance share its queue of content updates (the first to adopt takes them
       all, `GestureEnded` included). Routing it through the editor's projection cache, or
       asserting one projection per block, is left for whoever finds a second one.
     - An editor that detaches without the runtime's goodbye (cancels for its open gestures)
       leaves its undo groups open in `History` until an undo or redo commits them (`close_groups`); its tentative
       gestures are cancelled by the detach itself.
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
