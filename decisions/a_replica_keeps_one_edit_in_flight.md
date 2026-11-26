# A replica keeps at most one edit in flight

A replica submits one edit and waits for the order to bring it back numbered. What it types
meanwhile waits in its unsent queue (`be-sdk-type-guest/src/replica.rs`), where
`LiveEdit::absorb_operation` folds it together (typing forward becomes one insert, backspacing
one delete), and goes out as one edit when the first comes back. Merging stops before the merged
operation would pass the submission limit.

## Why

- It keeps the list of unconfirmed edits, and so the work of reprojecting the replica's view
  when someone else's edit lands beneath them, to one or two edits however slow the link is.
- It sends one submission per round trip instead of one per keystroke, so the server numbers,
  journals and broadcasts far fewer entries.
- Stopping at the submission limit sends a run of large pastes in pieces the server takes.
