# Backlog

Depends on: none.

The running to-do list, grouped. Every item says what "done" looks like.
Fixed items are removed as they land.

## 2. Docking

- **Bar drops across layers.** A joined dock bar (guides/beui/docking.md,
  joined groups) dropped across layers only reorders within its own layer.
  Done: it can be dropped on any edge of any joined layer's pane, shown by a
  dock test.

## 7. Sessions

- **Catch-ups from viewers.** `Catchup` is a `peer` relay, so any viewer can
  send one, and each makes the owner answer a `Snapshot` (with session state
  unless the asker already holds it) and may set it to reload. Done: the owner
  answers one sender at most once per interval, shown by a be-sdk-type-guest
  test where a viewer's repeated `Catchup`s get one `Snapshot`.

## 10. Search

- **Idle compaction on the device's idle and power.** The search index's idle
  compaction (guides/blocks/host.md, Search) waits only for the stack's own
  idleness, as nothing tells be-host-stack the user is away
  (`Windows::idle` stays in be-host) or that the device is on power. Done: the
  host passes both to the stack, and the pass waits for them, shown by a stack
  test where an index over the cap is left alone on battery.

## 11. Large folders

- **Republishing a large folder's summary.** A folder's summary lists every
  entry (up to `MAX_REFERENCES`), and every change to it is announced to each
  watching connection with its whole summary (`BlockChanged`), so adding N
  entries one at a time sends O(N²) bytes to each watcher. Done: a change
  announces only what moved in its references (as `publish` sends a
  `ReferenceChange`), and a test adding thousands of entries to a watched
  folder sends bytes linear in them.
- **What the hub keeps of a large folder.** `WatchHub` keeps, per connection,
  the `Facts` of every block it told the connection of, references included
  (`watch.rs`), so each connection listing or reading a folder of
  `MAX_REFERENCES` entries holds a copy of its whole reference set beside the
  graph's. Done: the hub reads references from the graph when it matches a
  change, keeping only what the connection was told that the graph cannot
  answer, and a server test shows its memory per connection does not grow with
  a folder's references.
