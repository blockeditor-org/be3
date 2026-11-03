# A follower keeps at most one edit in flight

A session follower sends one edit and waits for the owner to confirm it.
What it types meanwhile waits in its unsent queue
(`be-sdk-type-guest/src/replica.rs`), where `LiveEdit::absorb_operation` folds
it together (typing forward becomes one insert, backspacing one delete), and
goes out as one edit when the confirmation arrives. Merging stops before the
merged operation would pass the relay limit.

## Why

- It keeps the pending list, and so the work of reprojecting the follower's
  view when someone else's edit lands, to one or two edits however slow the
  link is.
- It sends one message per round trip instead of one per keystroke.
- Stopping at the relay limit sends a run of large pastes in pieces the owner
  can relay.
