# Text on the shared sequence

Text blocks keep their content as a `Vec<u8>` with positional operations that
`transform` rebases, a line diff3 for offline merges, and an undo stack of their
own inside the plugin. Model blocks have `List`, whose items have ids, whose
inserts anchor to a sibling, and whose undo is conditional. This plan moves text
onto one ordered sequence that lists can share, without saving any of its
bookkeeping.

## Decisions

- **Saved form is the payload only.** A text field saves its bytes, a list saves
  its `ObjectId`s. Position ids, fragments and tombstones exist only in the live
  session. Partial reads and chunk
  deduplication keep working on plain bytes.
- **Live edits address positions; offline merges address payloads.** Positions
  are never saved, so an offline merge cannot match them: text merges with line
  diff3 (conflicts stay visible, as today), lists with `merge_order`. Inline text
  fields inside other models (titles, cells) stay `String` registers for now,
  which keep ours and count a conflict; a register that keeps both sides of a
  conflict is wanted later, for the VCS case where only one side is right (a
  password).
- **Pure bytes.** Text positions are bytes with no UTF-8 awareness, so hex mode
  can edit anything. Anything that turns text into a `String` uses a lossy
  conversion. The editor must handle invalid UTF-8 arriving from another peer;
  the previous editor did, so this is preserving existing behaviour.
- **One true order, no CRDT.** The session owner sequences every operation, so
  operations apply in one order everywhere and nothing is rebased.

## Positions

A position is `(client, offset)`. Each client appends everything it inserts in a
session to one buffer of its own, wherever in the document it types, so offsets
are unique without coordination (this is Yjs's `(client, clock)`). The content
the session loaded is client `0`; the server numbers real clients from 1.
`ClientId` is per connection, so a reconnect starts a new buffer, and an insert
resubmitted after a reconnect still names its old client: the owner does not
check that a client only appends to its own buffer.

An insert names its first offset explicitly (`start`), and is refused unless it
is the next offset of that client's buffer. That never fires in normal use
(operations are deduplicated by `OpId` and arrive in order per client); if it
does, the edit is visibly lost instead of two elements sharing an id. An insert
or replace with the right `start` uses up its offsets even when it is refused
for another reason (a replace whose text someone else changed first, an anchor
nobody knows), so the client's later inserts, numbered as if it had applied,
still land. Without that, losing one replace race lost everything the client
typed after it.

## Structure

The order is a list of fragments, each a slice of one client's buffer with a
visible flag (false is a tombstone). An insert that continues the fragment
before it, in the document and in id, extends that fragment, so typing stays one
fragment. Fragments split by deletes are not joined back up; they go when the
session resets. Fragments are
kept in chunks with visible counts, and an index from each fragment's first
position to its chunk, so finding a position or a visible offset does not walk
the whole document. (A sum tree, like Zed's, would make it logarithmic; chunks
are enough until profiling says otherwise.)

## Operations

- `Insert { after, client, start, items }` lands immediately after `after`
  (or at the start). Of two inserts after the same element, the later-ordered
  one is first. An unknown anchor refuses the insert rather than appending it.
- `Delete { spans }` hides exactly the positions the deleter saw; text another
  peer inserted inside the selection at the same time survives.
- `Undelete { spans }` shows them again with their original ids. It is the undo
  of a delete and the redo of an insert, so anchors on restored text keep
  working.
- `Replace { spans, client, start, items }` is all-or-nothing: it applies only
  if every span is still visible, then hides them and inserts the items after
  the last of them. Two people checking the same markdown box produce `[x]`,
  not `[xx]`; the editor asks for it with `DocumentEdit::replace_atomically`,
  which toggling a checkbox uses. Typing over a selection stays a delete and an insert, so a
  collaborator's edit inside the selection does not drop what you typed.
- `Swap { hide, show }` applies only if everything in `hide` is visible and
  everything in `show` hidden. It is the undo and redo of a replace.
- `Move { first, last, after }` moves the range between two positions,
  tombstones and concurrent inserts inside it included, so edits anchored inside
  it go with it. A range whose ends are out of order, or a destination inside
  the range, changes nothing.

Applying an operation reports the visible splices it made, so an editor and a
highlighter get exact ranges instead of diffing the document.

## Undo

Undo stays client-local and conditional. An insert's undo deletes its span, a
delete's undo undeletes what it hid, a replace's undo swaps back, a move's undo
moves the range back after the element that preceded it. Undoing a move is not
conditional: it moves the range back even if someone moved it since, and redo
moves it to where the move put it; lists do the same. A burst of inserts (or
of deletes) into one field undoes as one step. History is capped at 200 steps
and 8 hours. Revision history is the way back past that.

## Sessions

- Every position-state reset starts a new session id. Operations carry it, and
  the owner refuses operations from another session; the follower then merges
  its unsaved bytes with diff3 (base: its confirmed text at the restart, ours:
  its visible text, theirs: the owner's). Translating stale operations is not
  worth its complexity for how rarely it happens.
- A session resets when everyone has closed the document, or after 8 hours with
  no edits (by then every undo step has expired, so nothing refers to old
  positions). That replaces the quiet-minute `Live::restart` of #339, and
  covers list removal records too.
- A document edited at least once every 8 hours and never closed by everyone
  never resets. Accepted for now.

## Reprojecting a follower's view

A follower shows `confirmed` plus its pending edits. When someone else's edit
arrives while it has pending edits, `Live` clones `confirmed`, replays every
pending edit, and journals `Replaced`; the worker then clears its operation log,
so the editor plugin gets a whole snapshot instead of the edit. Measured on a
1 MB text with two clients typing 200 characters each before catching up:

- The rebuild costs incoming edits times pending edits, not document size:
  200 rebuilds took 61 ms, of which cloning the document is about 60 µs each.
  With ordinary latency a typist has two or three pending edits, so a rebuild
  is around 100 µs.
- The snapshot is the expensive part: 3 ms to encode, 1 MB of bytes plus 1 MB
  of session state (it repeats the loaded text), 6.5 ms to decode and adopt in
  the plugin, then every watcher reruns and the editor diffs the document.
  Over 10 ms per keystroke from a collaborator, whenever you have anything
  unconfirmed.

A follower keeps at most one edit in flight. What it types before the owner
confirms that edit waits in `Live::unsent`, where `LiveEdit::absorb_operation`
folds it together (typing forward becomes one insert, backspacing one delete),
and goes out as one edit when the confirmation arrives. Merging stops before the
sum of the parts' encoded sizes would pass the relay limit, so a run of large
pastes goes out in pieces the owner can relay. That keeps the pending
list, and so the rebuild, to one or two edits however slow the link is, and
sends one message per round trip instead of one per keystroke.

Fixes, best value first:

1. **Skip the rebuild when the incoming edit commutes with the pending ones.**
   Apply it straight to `visible` and journal it as `Applied`, so plugins keep
   receiving operations. A `LiveEdit::commutes(pending, incoming)` hook that
   defaults to false. For text: the incoming edit is an insert, delete or
   undelete; its insert is not anchored on the same position as a pending
   insert; and nothing pending is a replace, swap or move. That covers nearly
   all concurrent typing; when it does not hold, order genuinely depends on the
   sequencer and the rebuild stays.
2. **Rebuild once per poll, not once per incoming edit.** `apply_accepted` marks
   `visible` stale and `poll` rebuilds it once, turning a burst of N incoming
   edits into one rebuild.
3. **Make the snapshot cheaper.** Session state now carries positions but no
   bytes (done). Serialising `Sequence<u8>` as one byte string instead of
   element by element should take most of the 6.5 ms decode away.

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
- An operation over 1 MB is not relayed: `Live::edit` saves it as a
  replacement commit, and `adopt_replacement` replays the unsealed operations
  onto the reloaded content. With text, those operations name positions in the
  old position space, and the reloaded content has fresh positions, so they
  would land on the wrong bytes. A replacement must start a new session id and
  merge unsealed work with diff3 instead. 100k lines of indent is over that
  limit.
- Cloning matters because a follower's rebuild clones `confirmed`; after a
  heavy edit that is 15 ms per incoming keystroke until the session resets.
  The commuting fast path avoids it; chunks behind `Arc` (copy on write) would
  make the clone proportional to the number of chunks.

## text-editor-core

Decided: text-editor-core depends on the `sequence` crate and works on
`Sequence<u8>` directly, instead of staying generic and having the text block
translate its anchors. `Sequence` lives in its own crate (serde only) so the
editor and beui's text inputs can use it without the block stack; `be-model`
wraps it as the `Text` field. Done, as follows:

- `DocumentRead::anchor` and `anchor_index` take and return `Pos`; a cursor or
  selection is a position that survives other people's edits.
  `deleted_anchor_index` becomes where a tombstone sits. `AnchorTable` goes.
- `ChangeLog` and `TextChange` come from the splices `apply` reports; the text
  block's whole-document Myers diff in `adopt` (and the `similar` dependency)
  goes.
- `TextBuffer`, the editor's standalone document, becomes a `Sequence<u8>` with
  one local client, so beui's inputs work like the text block without a session.
- Undo: the text block uses `be-model`'s `Step`. Beui's inputs have no
  `be-model`, so a small undo stack over `Sequence::inverse` lives beside the
  sequence; both follow the same rules and the editor still works outside this
  project.

## Fuzzing

`sequence::fuzz::sequence(data: &[u8])` (behind the `sequence` crate's
`fuzzing` feature, and in tests) reads any byte string as a script: three clients edit their own views
(including undo, inserts after tombstones and garbage operations), a sequencer
orders what they send, and clients catch up the way `Live` does, rebuilding from
`confirmed` plus pending when someone else's edit lands under theirs. Every
sequenced operation is checked against a naive reference model (order,
tombstones, offsets, splices), every replica against the sequence's internal
invariants, and at the end every replica against the sequencer's state. Clients
type and backspace at a caret as well as at random places, and fold what they
have not sent yet with `SeqOp::absorb`, as a `Live` follower does; each merged
operation is checked against its parts applied one by one (positions, items and
next offsets). It panics on any violation. Test and fuzzing builds use two fragments per chunk so
the multi-chunk paths run constantly.

A test feeds it seeded random bytes, and `crates/sequence/fuzz/sequence.rs` is
a coverage-guided libFuzzer target over it, which `./scripts/buck run //:fuzz`
runs in turn with the others until it is stopped, keeping its corpus and crashes in
`target/fuzz/sequence/sequence`. A crash it finds becomes a test in
`crates/sequence/src/tests`.

## Status

Done:

- `Sequence<T>` in the `sequence` crate: positions, fragments, chunks, the six
  operations, splices, inverses for undo, an optional mirror of the visible
  items for editors, and session state that carries positions but no content.
  Content is kept as runs of the bytes this peer has seen; the visible bytes
  come from the receiver's own document when it adopts state, and an undelete
  or swap carries the bytes it shows, so a deleted run never has to be shipped.
- A `Text` field for model documents (`be-model`): saved as bytes, edited with
  `Change::Text`, undone through `Step` (bursts absorb), merged with line diff3,
  and carried in `Document::session_state`.
- The text block is `Document<TextBlock { language, indentation, body: Text }>`,
  registered with history, so undo is the worker's. text-editor-core anchors
  cursors to positions (`AnchorTable` is gone) and its `TextBuffer` is a
  mirrored sequence with an undo stack of inverses. The text block keeps a
  mirrored copy of the body, applies foreign edits to it from the projection's
  log and reports their splices to the editor; after a rebuild it re-reads the
  body and diffs only the common prefix and suffix. Peers' carets resolve,
  because positions are shared.
- `Live`: the quiet-minute restart is gone, so a session's positions last until
  everyone closes it. `Snapshot` carries the state as of the owner's last seal,
  and adopting state rebuilds a follower's view from `confirmed` and its pending
  edits.
- Lists: `Value::List` holds `Items`, a `Sequence<ObjectId>` in which every
  object sits at the position its id names (`client` is the id's low 64 bits,
  `offset` 0), so changes keep addressing objects and anchoring to siblings by
  id and editors are unchanged. A list has one slot per object: a removal
  leaves a tombstone, an object that comes back to a list undeletes its slot
  and moves it after its anchor, a move within a list keeps the slot, and a move
  to another list leaves a tombstone behind. A list without tombstones adds
  nothing to the session state; the removal records (`gone`) and `MoveIf` are
  gone. Two ids with the same low 64 bits can't both be live in one list: the
  second insert or move there is refused. `be_model::fuzz::lists` (a seeded
  test and the coverage-guided `//crates/be-model:fuzz-lists`) runs three
  clients inserting, removing, moving, undoing and reloading a board of columns
  and cards through a sequencer, and checks the tree's structure after every
  operation and that every replica, tombstones included, ends the same.

Still to do:

- Session ids in the protocol (who mints them: the server's registry or the
  owner), refusal of other sessions' operations, the diff3 fallback, and the
  8-hours-idle reset. Until then a session's positions only reset when everyone
  closes the block.
- A follower's replacement (an edit over 1 MiB) replays the owner's unsealed
  operations onto the reloaded content: BE3-184. With text that can apply
  position edits from the old content to the replacement.
- Range moves of several list items (`SeqOp::Move` already moves a range;
  nothing builds one for a list yet).
- The first two reprojection fixes above (commuting fast path, one rebuild per
  poll), and serialising `Sequence<u8>` as one byte string.
- Splices reported through `Touched` for editors other than the text block.
- A replace inserts after the last element of its last span as listed; it
  should insert after whichever replaced element is last in the document now,
  in case a move reordered them.
- The worker's undo history capped at 8 hours as well as 200 steps. Editor
  history groups (`finish_history_group`) are not passed to the worker, which
  groups by time.
