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
  test where an index over the cap is left alone on battery. Until then the pass
  is capped at `IDLE_MERGE_BYTES` (128 MiB); with both signals it could merge
  larger tiers, once a merge no longer reads its segments whole into memory.
