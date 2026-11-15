# Backlog

Depends on: none.

The running to-do list, grouped. Every item says what "done" looks like.
Fixed items are removed as they land.

## 1. Text

- **A long text's first row heights.** Opening a long Markdown text far from
  its top (a restored scroll), the rows around a fenced code block above the
  view are first placed tens of points apart from where they settle a layout
  later, so the scrollbar's thumb jumps as they are measured (the line at the
  top now stays put: `VirtualList`'s `anchor`). Find whether the list places
  them by its estimate before measuring them or a row measures short at first.
  Done: the first layout places them where they settle, shown by a beui test.

- **Anchored rows on the web without `overflow-anchor`.** On the HTML core a
  `VirtualList` leaves its `anchor` to the browser's scroll anchoring, which
  Safari (and so every iOS browser) lacks, so rows measured anew above the view
  move what is shown there. Done: the HTML list keeps its anchor row in place
  itself where `overflow-anchor` is missing, shown by a test on the HTML core.

## 2. Docking

- **Bar drops across layers.** A joined dock bar (guides/beui/docking.md,
  joined groups) dropped across layers only reorders within its own layer.
  Done: it can be dropped on any edge of any joined layer's pane, shown by a
  dock test.

## 3. beui platforms

- **A share sheet.** beui has no way to hand a link to the platform's share
  sheet, so the Invite member dialog offers copy and a QR code only. Done: a
  beui call that shares a `Clip` where the platform has a sheet (the web's
  `navigator.share`, Android's send intent) and says whether it has one, with
  a Share button in the Invite member dialog where it does, and in beui-demo.

## 7. Sessions

- **Catch-ups from viewers.** `Catchup` is a `peer` relay, so any viewer can
  send one, and each makes the owner answer a `Snapshot` (with session state
  unless the asker already holds it) and may set it to reload. Done: the owner
  answers one sender at most once per interval, shown by a be-sdk-type-guest
  test where a viewer's repeated `Catchup`s get one `Snapshot`.

## 9. Version control's caches

- **Walks that read thinning as a root.** Depends on #686, which rewrites
  be-block-session's Resume (or its builder confirming `resume.rs` is untouched).
  Needs a third, unreached outcome carried through Resume. `CommitStore::common_ancestor` and
  `is_ancestor` treat a commit the store lacks as having no parents, so where
  the server thinned history away (one commit a minute in a block's first
  hour) they can answer "no shared history" or "not an ancestor" for heads
  that share one; their callers are in `be-block-client/src/peer/offline.rs`
  and `be-block-session/src/resume.rs` (after #686). `MergeBaseWalk` already
  tells a gap from a root (`Unreached`). Done: both tell a gap apart and their
  callers fall back as the merge base walk does, shown by a be-block-commit
  test with a middle commit missing.

## 10. Commit times

Depends on #686 (session catch-ups replaced by server ordering).

- **A refused far-future commit is set aside, not sent.** A device stamps
  commits by the server's clock it measured, kept in its refs across restarts
  (`Peer::server_now`), but one that has never connected stamps by its own
  clock, and the server refuses a time more than `MAX_COMMIT_AHEAD` ahead as
  `CommitAhead` (guides/blocks/server.md, limits). The drain sets such a commit
  aside with its refusal. Re-stamp it instead: measure again and rewrite the
  unsent commit (and any unsent ones on it) with a new time, which changes
  their ids, so the journal, pending entries and local heads that name them
  move too. A first try (branch `backlog-10-times`) was held back for #686,
  which rewrites how replicas track their base and how the journal moves. Its
  review found what the design must cover: an open replica still holding the
  old id as its head or journal base merges with its own renamed copy (for an
  offline-made block, likely doubling its text), so the replica must be told
  of the rename; a seal racing the re-stamp, so `Refs::restamp` must
  compare-and-swap the heads it moves; checkouts' recorded heads; and the
  graph mirror. Done: a client test whose first commits are made offline with
  a clock two hours fast sends them once it connects, and a host test with the
  block open shows its text once.

## 11. Search

- **Idle compaction on the device's idle and power.** The search index's idle
  compaction (guides/blocks/host.md, Search) waits only for the stack's own
  idleness, as nothing tells be-host-stack the user is away
  (`Windows::idle` stays in be-host) or that the device is on power. Done: the
  host passes both to the stack, and the pass waits for them, shown by a stack
  test where an index over the cap is left alone on battery. Until then the pass
  is capped at `IDLE_MERGE_BYTES` (128 MiB); with both signals it could merge
  larger tiers, once a merge no longer reads its segments whole into memory.

## 12. Large folders

- **The rest of a large folder's change.** A change of a folder now reaches a
  watcher as what it did to the references, but each end still does O(N) work
  per change: the client's refs write the block's whole summary row on every
  change (`Refs::keep`, then `Peer::note`), so a watcher seeing N entries added
  one at a time writes O(N²) bytes to disk, and the server's `graph_views`
  clones every reference into each member's summary only for `WatchHub::tell`
  to empty them. Done: the refs keep a block's references apart from its
  summary row and write only what a change moved, the server makes a summary
  without references for a change it tells as `moved`, and a client test
  adding thousands of entries to a watched folder writes refs bytes linear in
  them.

## 13. Settings

- **Typed organization policies.** Each row of the organization's and the
  offline policies (be-app-settings `src/root/organization.rs`) is a `Policy`
  whose select binds a position into a fixed table of labels, read and written
  through `get` and `set` closures that index parallel arrays of values. Done:
  a policy's select binds the policy's own value (`Allowance`, `WhoMay`, days,
  a quota and so on), each option naming its value beside its label, with no
  position or parallel array left, and the organization's settings tests still
  pass.

## 14. Notifications

- **An app icon for notifications.** Android shows a pushed notification with
  its built-in `stat_notify_chat` icon, as the app has none of its own
  (`crates/be-host/android`). Done: the APK carries a monochrome notification
  icon of its own and `PushRaise` uses it.

## 15. Flaky host tests

- **Two host tests that failed once under verify load.**
  `a_joined_blocks_tabs_carry_its_menu` waited for "Referenced by (2)" and saw
  (1); `a_device_that_loaded_my_files_adds_to_it_offline_and_the_other_sees_it_once_it_reconnects`
  timed out in `synced` with "6 edit(s) wait to be sent". Neither failed in
  120 and 80 runs under `stress` (at 14 at a time, and with `--latency 60`).
  Their next failure explains itself: `synced` names each unsealed block, each
  operation in the outbox and the drains, and the host log says when a
  listing's answer left out a block the device held as listed there
  (`log_unlisted_held`); with no such line, the missing reference is past the
  graph, in what the app's `Backrefs` watch was told. Done: the cause found
  from that output and fixed at its source, with a test that shows it.
