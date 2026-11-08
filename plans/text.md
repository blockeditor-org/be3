# Text on the shared sequence

Depends on: none.

Text and lists share one ordered sequence: `Sequence<T>` in `be-lib-sequence`,
wrapped by `be-block-model` as the `Text` field and as `Items` for lists. The
session code is `be-sdk-type-guest/src/replica.rs`. What is left is below.

## A replacement carries pending edits over

An edit over `LARGEST_RELAYED_OPERATION` (1 MiB) is saved as a replacement
commit, and `replacement` in replica.rs replays the owner's unsealed operations
onto the reloaded content (BE3-184). With text, those operations name positions
the reloaded content no longer has, so they can land on the wrong bytes.
Carry them over as a session restart does
(decisions/sessions_restart_when_their_state_outgrows_the_content.md):
`LiveEdit::carry_over` moves each one onto the reloaded content as the splices
it made, on the owner and on every follower. Done: a test edits text, makes an
oversized edit before the owner seals, and every replica ends with the same,
intended bytes.

## Undo groups from the editor

beui-text-editor-core calls `finish_history_group` to end an undo step, but
`LiveText` (`be-sdk-text/src/live_text.rs`) ignores it, so the replica groups
text edits by time alone (`EDIT_BURST`). Map the editor's groups onto the
gesture calls (`ContentProjection::begin_gesture`, `commit_gesture`). Done: an
undo after typing, a pause shorter than `EDIT_BURST` and a paste undoes the
paste alone when the editor ended a group between them.

## Sequence

- A replace inserts after the last element of its last span as listed; it
  should insert after whichever replaced element is last in the document now,
  in case a move reordered them.
- Range moves of several list items: `SeqOp::Move` already moves a range;
  nothing builds one for a list yet (`Items::shift` moves one slot).
- Splices reported through `Touched` (`be-block-model`, today `Everything`,
  `Field` and `Subtree`) for editors other than the text block.
- Serialise `Sequence<u8>` as one byte string instead of element by element
  (`impl Serialize for Sequence<T>`); a snapshot of a 1 MB text took 6.5 ms to
  decode and adopt, most of it this.
- Inline text fields inside other models (titles, cells) are `String`
  registers, which keep ours on an offline merge and count a conflict. A
  register that keeps both sides of a conflict is wanted later, for the VCS
  case where only one side is right (a password).

## Not planned here

- Optimizing for one large edit, such as indenting every line of a large file
  (an insert at every line start). It is a bad case by nature and rare; it
  gets faster as the sequence does in general, and it should not shape the
  sequence's design.
