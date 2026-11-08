# Text on the shared sequence

Depends on: plans/large_blocks.md (the operation that adds a part by its
hash), for the oversized edit only.

Text and lists share one ordered sequence: `Sequence<T>` in `be-lib-sequence`,
wrapped by `be-block-model` as the `Text` field and as `Items` for lists. The
session code is `be-sdk-type-guest/src/replica.rs`. What is left is below, in
order.

## 1. Undo groups from the editor

The replica groups edits into undo steps by time: an edit within `EDIT_BURST`
(750 ms, `be-block/src/history.rs`) of the last joins its step. An editor can
also bracket a gesture (`LiveContent::begin_gesture`, `commit_gesture`,
`cancel_gesture`, sent as `FromEditor::Gesture`), such as a drag, which makes
everything between one step. beui-text-editor-core calls
`finish_history_group` to end a step where a text editor expects one (a paste,
a caret jump between typing), but `LiveText`
(`be-sdk-text/src/live_text.rs`) ignores it, so text steps follow the clock
alone.

Add `Gesture::Break`, which closes the current step (`History::close`) without
opening a gesture, and send it from `LiveText::finish_history_group`. Not
begin and commit: a gesture left open blocks session restarts (`restart_due`,
decisions/sessions_restart_when_their_state_outgrows_the_content.md). Done: in
a `BeuiTest`, typing, a pause shorter than `EDIT_BURST` and a paste, with the
editor ending a group between them, undo the paste alone and then the typing.

## 2. Sequence

Land these and plans/chat.md's `merge_appends` one after the other: both change
`Items` merges. A change to what an operation does is fine across builds, since
a journal belongs to its module tag.

- **A replace after a move.** A replace inserts after the last element of its
  last span as listed; it should insert after whichever replaced element is
  last in the document now. Done: a test moves an element of a span, then
  replaces the span, and the insert lands after the element now last.
- **Moving a range of list items.** `SeqOp::Move` already moves a range;
  nothing builds one for a list (`Items::shift` moves one slot). Add
  `Items::shift_range(first, last, after)`. Done: a test moves three items in
  one operation, and undo puts them back.
- **Saving text as one byte string.** `impl Serialize for Sequence<T>` writes
  element by element. Rust has no specialisation, so serialise `Value::Text`
  (`be-block-model/src/lib.rs`) through a bytes adapter instead. Postcard
  writes both the same way, so the stored form does not change; the gain is
  decoding. Done: decoding and adopting a 1 MB text's snapshot takes under
  1 ms in release (6.5 ms today), measured by a bench.

## 3. An oversized edit is an operation like any other

Today an edit over `LARGEST_RELAYED_OPERATION` (1 MiB, a large paste) is not
relayed. A follower stashes a commit holding its whole content and tells the
owner `Moved { head }`; the owner reloads that head and `replacement` replays
all its own unsealed operations onto it (BE3-184). Those operations name
positions the reloaded content does not have, since loading gives fresh
positions, and some may already be in the follower's commit, so text can land
on the wrong bytes or twice.

Send an oversized operation like any other instead: its payload is uploaded as
an object and the operation is relayed by its hash (the operation that adds a
part by its hash, plans/large_blocks.md), so the owner sequences it and every
replica applies it at the positions it names. The oversized path through
`Moved`, `Then::Absorb` and `replacement` goes; `replace_content` stays for
real replacements. Done: a test where a follower pastes over 1 MiB while the
owner has unsealed edits ends with every replica holding the same, intended
bytes, and undo removes the paste alone.

## Not planned here

- Optimizing for one large edit, such as indenting every line of a large file
  (an insert at every line start). It is a bad case by nature and rare; it
  gets faster as the sequence does in general, and it should not shape the
  sequence's design.
- Inline text fields and splices for other editors:
  plans/deferred/text.md.
