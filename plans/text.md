# Text on the shared sequence

Depends on: none.

Text and lists share one ordered sequence: `Sequence<T>` in `be-lib-sequence`,
wrapped by `be-block-model` as the `Text` field and as `Items` for lists. The
session code is `be-sdk-type-guest/src/replica.rs`. What is left is below.

## Sessions

Today a session's positions only reset when everyone closes the block.

- Every position-state reset starts a new session id. Operations carry it, and
  the owner refuses operations from another session; the follower then merges
  its unsaved bytes with diff3 (base: its confirmed text at the restart, ours:
  its visible text, theirs: the owner's). Translating stale operations is not
  worth its complexity for how rarely it happens. Open: who mints session ids
  (the server's registry or the owner).
- A session resets when everyone has closed the document, or after 8 hours with
  no edits (by then every undo step has expired, so nothing refers to old
  positions). This covers list removal records too.
- A document edited at least once every 8 hours and never closed by everyone
  never resets. Accepted for now.
- A replacement (an edit over 1 MiB, `LARGEST_RELAYED_OPERATION`) is saved as a
  replacement commit, and `replacement` in replica.rs replays the owner's
  unsealed operations onto the reloaded content: BE3-184. With text, those
  operations name positions in the old position space, and the reloaded content
  has fresh positions, so they can land on the wrong bytes. A replacement must
  start a new session id and merge unsealed work with diff3 instead.

## Undo

- The worker's undo history (`HISTORY_STEPS` in `be-block/src/history.rs`) is
  capped at 200 steps; cap it at 8 hours as well, which the session reset above
  relies on.
- Editor history groups (`finish_history_group` in beui-text-editor-core) are
  not passed to the worker, which groups by time (`EDIT_BURST`).

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

## Indenting a whole file

Select-all then Tab is one edit with an insert at every line start, so it splits
every line's fragment. Measured in release, lines of about 40 bytes:

| | 10k lines (390 KB) | 100k lines (3.9 MB) |
| --- | --- | --- |
| apply on each peer | 13 ms | 250 ms |
| build the edit, take its undo step | 18 ms, 18 ms | 390 ms, 340 ms |
| operation size | 290 KB | 3 MB |
| fragments after | 20k | 200k |
| one keystroke after | 85 µs | 100 µs |
| cloning the document after | 0.9 ms | 15 ms |

Each indent and unindent adds a tombstone per line (600k fragments after four
rounds at 100k lines). The time is the per-insert walk over chunks (cost grows
faster than the line count), the size is a `Change::Text` with its object id per
line. Fixes, if it matters:

- A Fenwick tree over the chunks' visible counts makes finding an offset and
  reporting a splice logarithmic, which is most of the apply time.
- One operation inserting the same client's bytes at many anchors (one buffer
  append, a list of `(after, len)`) instead of a change per line, which also
  means `Document::step` takes one change rather than cloning the tree for a
  multi-change edit.
- 100k lines of indent is over the relay limit, so it goes through the
  replacement path above.
- A follower that rebuilds its view (`rebuild_visible`) clones `confirmed`;
  after a heavy edit that is 15 ms each time. Chunks behind `Arc` (copy on
  write) would make the clone proportional to the number of chunks.
