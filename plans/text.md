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
  session, like the removal records of #339. Partial reads and chunk
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
does, the edit is visibly lost instead of two elements sharing an id.

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
  not `[xx]`. Typing over a selection stays a delete and an insert, so a
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
moves the range back after the element that preceded it. A burst of inserts (or
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

## Status

Done (in `be-model`, not used by any block yet):

- `Sequence<T>` with positions, fragments, chunks, the six operations, splices,
  inverses for undo, and session state.
- A `Text` field for model documents: saved as bytes, edited with
  `Change::Text`, undone through `Step` (bursts absorb), merged with line diff3,
  and carried in `Document::session_state`. Objects inserted with a text field,
  and documents produced by a merge, start from fresh positions so every peer
  agrees on them.

Still to do:

- `List` on `Sequence<ObjectId>`: moves inside a list keep positions, a move
  between lists is a delete plus an insert carrying the same `ObjectId`, and the
  removal records become tombstones. Range moves of several items come with it.
- Session ids in the protocol (who mints them: the server's registry or the
  owner), refusal of other sessions' operations, the diff3 fallback, and the new
  restart triggers.
- `Live` adopts session state into both `confirmed` and `visible`. With text,
  `visible` holds pending edits whose positions the owner's state lacks, so it
  has to be rebuilt from `confirmed` plus the pending operations instead.
- The text block as `Document<TextBlock { language, indentation, body: Text }>`,
  with text-editor-core reading fragments directly, anchoring to positions, and
  dropping its own undo, `AnchorTable` and whole-document diff. Beui's own text
  inputs use text-editor-core too, so the sequence may need to move into a small
  crate both can depend on.
- Splices reported through `Touched` (or beside it) to editors.
- The worker's undo history capped at 8 hours as well as 200 steps.
