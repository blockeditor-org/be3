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

- **A row's rename that outlives its moment.** be-app-file-tree's `Renaming`
  keeps a `pending` rename of a new block until its row shows, and a started
  one (`editing`) until its field finishes, even after the tree lost the
  keyboard to another block. Done: a pending rename is dropped, and a started
  one ends as left, once the tree's keyboard goes to another block, shown by a
  file tree test.
- **Renaming from a stacked bar's title.** On a phone a new block is named from
  its title in the stacked bar, which shows no cue that a tap renames it.
  Done: the title reads as renamable (a cue the styled dock draws), shown in a
  painting of the stacked bar.

## 2. Docking

- **Bar drops across layers.** A joined dock bar (guides/beui/docking.md,
  joined groups) dropped across layers only reorders within its own layer.
  Done: it can be dropped on any edge of any joined layer's pane, shown by a
  dock test.
- **A clipped tab strip says so.** A pane's tab strip scrolls and clips tabs
  past its edge with no cue that more lie there (the inspector at its default
  width hides its last tabs). Done: a clipped edge fades, or an overflow menu
  lists the hidden tabs, shown by a painting.

## 3. beui platforms

- **A share sheet.** beui has no way to hand a link to the platform's share
  sheet, so the Invite member dialog offers copy and a QR code only. Done: a
  beui call that shares a `Clip` where the platform has a sheet (the web's
  `navigator.share`, Android's send intent) and says whether it has one, with
  a Share button in the Invite member dialog where it does, and in beui-demo.

- **A view shown again on the web.** A dock tab or bar shown again (a text's format bar after
  a tab switch) can draw empty for a frame in a browser: the native host keeps a hidden view's
  last frame until its app draws it anew (guides/apps/runtime.md), but the web's surfaces are the
  app worker's canvases and it keeps none. Done when the web keeps a hidden view's last canvas
  frame the same way, with a web test that shows it again before its app answers.

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

- **A joined block's tabs, once under verify load.**
  `a_joined_blocks_tabs_carry_its_menu` waited for "Referenced by (2)" and saw
  (1), and has not failed since in 240 runs under `stress`. Its next failure
  explains itself: the host log says when a listing's answer left out a block
  the device held as listed there (`log_unlisted_held`); with no such line, the
  missing reference is past the graph, in what the app's `Backrefs` watch was
  told. Done: the cause found from that output and fixed at its source, with a
  test that shows it.
- **Adding to My files offline, about once in 400 runs.**
  `a_device_that_loaded_my_files_adds_to_it_offline_and_the_other_sees_it_once_it_reconnects`
  failed 3 times in 1250 runs under `stress` (12 at a time, `--latency 60`),
  each a different way: busy on an editor's view block whose edits were not
  journaled and owed no seal, at a click in the setup; the other device's files
  pane loading for good while it waited for the block added offline; and, after
  reconnecting, six publishes of one block left in the outbox through each
  drain. Each failure now names its cause: a replica's `unjournaled` in the busy
  reason (a timer a stopped clock never brings, or stuck work), whether the
  failed test's host is in step and with what, and what the last drain answered
  of what it left. Done: each cause found from that output and fixed at its
  source, with a test that shows it.
- **A settings page beside the settings list, painting one glyph two ways.**
  `a_settings_page_and_the_settings_list_show_the_same_block_side_by_side`
  (be-host-tests) painted the glyph over [877, 14, 889, 26] in [230, 235, 243]
  in one run of verify and in [141, 153, 174] when run again: one of the two
  colours a hovered or focused control shows, as if the pointer or the focus
  sits on it in one run and not the other. Done: the cause found (what the
  test reads that varies, guides/testing_a_gui.md, 6) and fixed at its source,
  so the painting is the same every run.
- **An app stopping under verify load on the build server.** In one full verify, four host
  tests (`the_more_card_hangs_on_its_tab_and_leaves_the_files_usable`, the canvas menu's and
  two of a canvas in a canvas) showed the session picker with "Workspace stopped: The app
  stopped responding.": a workspace step ran past `STEP_TIMEOUT` (10 s). The failing test used
  8.0 s of CPU in 35.9 s, 7.1 s of it system against 1.0 s user, where a passing run uses
  about 2 s with 0.3 s system: starved, by what looks like memory reclaim on a crowded worker,
  not hung. None failed in 120 runs under `stress` on a 4-core VM. A wait now names such a
  stop (guides/testing_a_gui.md, 6). Done: the build server runs host tests no more at once
  than its memory holds, and a full verify passes with none of them stopped.

- **A reopened canvas writes its default bars over its saved ones.**
  `the_canvas_zoom_bar_floats_at_any_corner_and_stays_there` failed 2 in 40
  under `stress` while editor frames reached the editor a host frame late; each
  failing log has the canvas's view block drop a child from its references
  ("dropped out of its parent's references, set by its open replica's report").
  Frames now go as they are handed over, which hides it, but any latency (a
  slow worker, the real network) brings it back. A suspect: `LayeredSettings`'s
  `ready` (be-sdk-beui `src/layered_settings.rs`) counts a layer block it
  doesn't know yet as loaded. Done: the race found and fixed in the app, with
  frames delayed a frame in a test to show it.

## 16. Host test tools

- **Stepping an app's present apart from the poll.** With the clock stopped,
  `drive` runs each frame only once the apps have answered, so an app's
  present and the `poll` that applies its held children always land in the
  same host frame. A host test can't show a frame drawn between them (a
  stacked dock's bar over the children it replaces); only the runtime's unit
  test `a_frame_whose_children_wait_for_the_poll_is_shown_with_them` does.
  Done: a `drive` step that lets an app's present arrive before the next poll,
  and a host test at phone size that opens a document from the files and
  shows each frame between.

## 17. Dragging

- **A carried item's own face, unclipped.** A carry with no `preview` floats as
  its source drawn displaced and raised, which a scroll around the source still
  clips (guides/beui/components.md, Drags as places). Done: the source's face
  captured as a texture on both cores floats in a top layer like a preview,
  so no carry needs `preview` just to leave its scroll.
- **A keyboard step reveals the place it chose.** Edge scrolling follows only a
  pointer, so a keyboard carry stepped to a place in the hidden part of a scroll
  leaves it out of sight. Done: after each keyboard step the board scrolls the
  chosen rest into view in every scroll around it, as focus does.

## 18. Keys

- **Commits sealed before a rotation.** A commit made offline is sealed under
  the key the device held when it wrote it. If the block's keys rotate
  meanwhile (a member removed), the drain publishes it under the old key, which
  the removed member still holds: neither `drain_publish` nor the server checks
  the key's epoch. Offline writes report `Saved::Kept`, version control's
  included, so this is the usual path. Done: the drain reseals such a commit
  (and those built on it) under the current key before sending it, or the
  server refuses one sealed under a superseded key (plans/signed_access.md's
  key epochs), shown by a client test where a member is removed while another
  device writes offline and the removed member cannot open what it sends.

## 19. Local accounts

- **Share and Access offered with no server.** A local account's block menus
  still offer Share and Access, which it refuses. Done: a local account's
  menus offer neither, shown by a host test of a local account's block menu.
- **Keeping a web local account.** On the web a local account lives only in the
  origin's storage, which the browser may evict. Done: making one asks the
  browser to keep it (`navigator.storage.persist()`), and says so when refused.
- **A server for a local account.** Done: a local account can be given a
  server, sending everything it holds there as a push to a new remote, and goes
  on as that server's account.

## 20. Keyboard

- **A view given the keyboard before it is ready.** When the host gives a
  view the keyboard before its content is up (a joined text's dock tab view
  given it the moment its group is shown again, before the app's subregion
  hears of the child placed in its stead), the view takes the host's focus but
  its app does not focus the field it would otherwise: the document's focus is
  already on another node inside that view's window, so neither the window's
  kept focus nor `focus_into` (which waits one layout) puts it on the text
  input. Found while fixing Ctrl+Tab between texts, which asks again only
  once the app knows the new child (`gift-missed`) and so avoids it. Done: a
  view given the keyboard early focuses its field once it is up, shown by a
  host test that gives a text's view the keyboard before its content arrives.
