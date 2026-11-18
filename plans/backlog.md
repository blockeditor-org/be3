# Backlog

Depends on: none.

The running to-do list, grouped. Every item says what "done" looks like.
Fixed items are removed as they land.

## 1. Text

- **A quoted code block's prefix.** A fenced code block inside a quote
  (`> ```) keeps its `> ` in a zero-width marker, so its caret positions and
  selection have no room there, unlike every other quoted line, whose row lays
  the prefix over the quote's gutter (`quote_in_text`). Its text sits inside
  the block's background frame in the line hook, and the code padding inside
  that frame depends on the margin, which the row cannot see. Done: the line
  hook draws the code background apart from the text, starting past the
  gutter, the row lays the quoted code line's prefix over the gutter with the
  code inset after it, and a text app painting shows carets at each prefix
  offset of a quoted code line.

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

- **The host's canvas when a page grows.** In headless Chromium, the first frame after the page
  grows still shows be-host's own chrome at the old width, with the page background beyond,
  although beui-platform-web runs a frame from its `ResizeObserver` (`resized` in web.rs). Next
  step: log whether `runner.frame()` has something pending and whether `present()` runs inside
  `resized`. Done when the first frame after a grow is drawn at the new size, shown by a web test.

- **An app view's backdrop beyond its last placement.** While a view grows before its app
  reports the new size, the part beyond its last placement shows what is behind the view (its
  placer's backdrop or the host's surface), not the app's clear colour, because `Backdrop` in
  `app_view.rs` fills the placement's rect. Filling the laid box with a styled `Frame` instead
  made `typing_right_after_adding_notes_reaches_the_notes` lose the typed text every time, even
  with a transparent `Frame`. Suspect: `take_keyboard` reads its catcher `NodeRef` untracked, so
  a view that wants the keyboard before the catcher exists loses it until `wants` changes, and
  an extra node changes when that happens; moving `take_keyboard` after the view is built did
  not fix it, so the cause is still open. Done when the backdrop fills the laid box, that test
  passes, and the keyboard loss has a test of its own.

- **One order for a press on both cores.** The native core (`beui-core` `interact/pointer.rs`,
  `press`) calls a node's `on_press` before `on_active_change(true)`, while beui-core-html arms
  it first. Value controls no longer depend on the order (each reports its drag from whichever
  comes first, `value_drag.rs`), but anything else reading `active` in `on_press` sees the two
  cores differ. Done when the native core arms before `on_press`, as the HTML core does, with a
  test on both cores.

- **A live resize on macOS.** `CAMetalLayer`'s default `contentsGravity` (resize) may stretch
  the last frame while a window is resized live, before beui draws the new size. Check it, and
  set it to top left if so, as X11 keeps the last frame at the corner (beui-platform-winit's
  `backdrop.rs`). Done when a live resize on a Mac shows the last frame unstretched.

## 7. Sessions

- **A diverged replica's journaled edits after it closes.** A replica that
  diverged, or whose seal cannot merge, seals nothing as it closes, so journaled
  edits no seal holds (as a held rejoin's, `rejoin_held`, be-block-type-host's
  replicas.rs) are left only in this device's journal; `ended` drops them from
  its open edits as the order holds them, and a replica opened online passes
  that journal over for the server's when there is one (`journal_for`). Whether
  to give them again on the content a reload replaces is a product call. Done:
  decided, and if they are kept, a test with a replica that really diverged
  shows them given again once after it closes and opens online.

- **A seal of its own during a join that holds kept edits.** `rejoined` seals
  and waits only for a join begun on a settled made-up session
  (`Open::joined_settled`). One that joins holding kept or doubtful edits takes
  the server's journal as it is; if its made-up session sealed on its own while
  the join was on its way (its seal timer, a close), that commit is pending in
  the outbox and the replica reopens beside it, as the settled case did before
  `sealed_joining`. It is expected to resolve once the drain sends the commit
  and the replica merges it, with the sealed edits given up and not given again.
  Done: a be-host-stack test that reopens a block after a crash, has its
  made-up session seal while the join is held, and counts exactly what was made.

- **A kept submission that does not encode again.** A replica opened again
  submits the edits a kept submission holds under its id only if they encode
  to its inline payload again (`Replica::flush`); otherwise they go as a new
  submission, and if the first copy reaches the server late, from the process
  that crashed, the order takes them twice. Two cases never match: an operation
  too large to relay goes by its part (`Payload::Part`), and an edit carried
  over a restart of the order goes again from `replay` as typed while its kept
  submission holds the operation carried over, which differs for a type whose
  `carry_over` rewrites it (text's positions). Done: such a submission matched
  by what it holds rather than its bytes (the part's hash; the edits it was
  carried from) and sent again under its id, with guest tests where its first
  copy lands after the one given again.

- **Kept edits across two crashes, end to end.** The tests of kept submissions
  across a restart of the order (be-block-type-host's
  `an_edit_carried_over_a_restart_keeps_the_identity_it_goes_again_under_across_a_crash`
  and `a_replica_opened_again_after_a_restart_is_told_it_and_its_edit_lands_once`)
  check what the host keeps and tells the replica, not a second copy landing
  late: that harness has no relay to hold a crashed process's submission, and
  cannot type into a reopened replica before it loads, so the sequence type,
  keep, crash, reopen, crash again is untested end to end. Done: a harness that
  holds a submission and controls when a replica loads, and a test of that
  sequence counting exactly what was made.

- **A lost seal's rebase with a merge above it.** Before
  plans/simpler_server_api.md step 1, the server could lose a seal's
  `rebase-journal` while the outbox published the seal and seals made on it
  since; `journal_for` finds the seal on the server journal's base by walking
  down the head's single-parent seals. A merge commit between them, or more
  than `SEAL_WALK` offline seals above it, stops the walk, so the replica opens
  on the journal's base and merges the head in on top of entries the head
  already holds, counting them twice. Step 1 closes the cause: a seal the outbox
  publishes carries its journal seal and trims the server's journal as it wins
  the head (be-host-stack's
  `a_seal_whose_publish_was_lost_trims_the_journal_when_the_outbox_drains_it`).
  The walk stays until no path is shown to reach it: a seal that wins the head
  untrimmed (a stale seal, one the host made in a made-up session) with entries
  past the journal's base it holds. Done: either such a path and a test that
  counts exactly what was made through it, or the walk removed with a test that
  shows every seal on the journal's base trims it.

- **The order's entries pushed out of order.** `submit` numbers a submission in
  the database (`journal::submit`) before `SessionRegistry::sequenced` takes the
  registry's lock to push it, so two submissions numbered at once can reach
  participants as N+1 then N, and a membership entry written between them is
  stamped with whichever was pushed last. Replicas hold what arrives early
  (`ahead`), and a host that sees a gap joins again, so nothing is lost, but the
  stream is not one order. Done: numbering and pushing under one per-session
  lock (the registry's lock is server-wide, so not that one), with a be-server
  test where many connections submit at once and every participant hears the
  entries in sequence.

- **A silent holder keeps the lease for 90 seconds.** A holder whose
  connection goes silent without closing stays the holder until be-server
  drops the connection at `IDLE_TIMEOUT` (90 s), longer than the 30 s the
  heartbeat lease took before plans/simpler_server_api.md step 1. The client
  pings every `PING_INTERVAL` (15 s) and gives up after `PONG_WAIT` (20 s), so
  a server limit of about 2 × `PING_INTERVAL` + `PONG_WAIT` (about 45 s) still
  keeps every live client. Done: the server's limit derived from the client's
  ping constants, with a be-server test that a silent connection's leave is
  written within it.
- **A terminal host stands by on a timer.** Since a claim takes the lease at
  once, a second terminal host waits `CLAIM_INTERVAL` (3 s) after it first sees
  a new holder before claiming, so the holder's presence can reach it first
  (crates/be-tool-cli terminal_host.rs). Under real load that presence can take
  longer, and the second host then steals the lease and the viewer's keys are
  lost. Replicas already send their presence to each joiner, so stand by until
  the holder's presence arrives or `CLAIM_INTERVAL` passes: at once in the
  common case, the timer only for a holder that sends no presence. Done: a
  test where the holder's presence is held back past the join, and the second
  host still stands by once it arrives.

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
- **A block indexed before its ancestor is listed.** Search's indexer meets a
  block whose ancestor the mirror has not listed: push-web-host-test's Ada logs
  "be-host: <block> is not indexed: the mirror cannot place it, as <parent> on
  its way up is not listed" four times (on 1c0ba8a5e, before the worker had a
  memory of its own, so not from it). Done: the cause found and fixed, with the
  line gone from that test's log.

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

- **The first text opened, above 2 hosts per core.**
  `unlinking_a_reference_in_a_text_puts_a_copy_in_its_place` timed out at its
  first `wait 'Pane "Text"'` 11 times in 192 under `stress` at 16 at a time on
  4 cores, and none in 96 at 8: the new text's row showed, its tab never
  opened, and nothing was busy but a frame the stopped clock held.
  `Runtime::busy` (be-host-runtime's runtime.rs) answers nothing for an app with
  no mounted view, so an app still starting may read as settled. Done: an app
  starting for a view the host shows keeps it busy, or the cause found
  elsewhere, and the test passes 96 at 16 at a time.
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
- **A stack test, under a full suite's load.**
  `discarding_the_unsent_edits_of_a_block_demoted_mid_edit_leaves_nothing_of_it_kept`
  failed once in a full `./scripts/test //crates/be-host-stack:test`, and once in
  8 runs of the whole test binary (the server refused an edit: "not editable by
  this account"); it fails 1 in 40 alone. Done: the cause
  found (under the suite's load, or `stress` at more at a time) and fixed at
  its source, with a test that shows it.

- **A note made offline, once under the render-props branch's tests.**
  `a_note_made_while_a_device_is_offline_reaches_it_once_it_reconnects`
  failed once in a `./scripts/test //crates/be-host-tests:test` run and passed
  on the rerun; its output was not kept. It passed 280 of 280 under `stress` on
  a 4-core VM (8 at a time, with every app and with `--apps text`, and
  `--latency 60`). Done: run it under `stress` until it fails, and fix the cause
  its output shows at its source, with a test that shows it.

- **Quick open's hit without its snippet, once under verify.**
  `quick_open_finds_a_word_typed_in_a_text_and_opens_it` timed out waiting for
  the snippet "ripen late" with its result row shown. It passed 460 of 460 under
  `stress` on a 4-core VM (8 at a time with every app, `--apps text --latency 60`,
  and 14 at a time with `--latency 100`). A suspect: be-host-stack's search
  caches a snippet's text by block, key and head (`cached_text`), failures
  included, so a `search-text` that failed once (`NotSearched`, a type module
  busy or starting under load) leaves that hit's snippet empty until its head
  moves. Done: the cause found from a failing run's output and fixed at its
  source, with a test that shows it.

- **A crash test that once never counted 11 after the restart.**
  `an_edit_to_a_block_made_editable_again_after_its_edits_were_discarded_lands_once_across_a_crash`
  (be-host-stack) hung once in 2000 local runs (16 at a time, the harness built once and run
  directly) at the `wait_for_count(block, 11)` after its restart, with nothing owed, undurable or
  draining. Whether it counted 1 (the edit lost) or 21 (ordered twice) was not recorded;
  `wait_for_count` now says what it last counted. Done: the cause found from that output and
  fixed at its source, with a test that shows it.
- **A take that returns before another's notices are told.** In a test that runs no frames,
  be-host-stack's pump thread (`start`) applies an update under the stack's lock and tells the
  watchers after it lets go (`take_updates`), so a `take_updates` on the test's thread meanwhile
  (`wait_until`'s) finds nothing left and returns before the watchers heard what the state now
  shows. be-host-runtime's passes take the updates as each starts, which stops the pump, but
  be-host-stack's own tests still race it. Done: a take that finds another under way returns only
  once that one has told its notices, shown by a stack test.
- **A retried listing sent late.** be-host-runtime's
  `a_listing_the_host_could_not_load_reaches_the_app_as_failed` failed once in a full verify
  (`once a retry loads it, the answer is sent again`: nothing was sent) and passes alone. A
  suspect: after `refetch`, `loaded` finds the graph's query ready before the refetch has
  marked it loading, so the test reads the app's answers before the new listing is sent.
  Done: the test waits on the refetched listing itself, and passes 100 runs under load.
- **A restarted app's editor that is never sent its snapshot.** When an app restarts,
  be-host-runtime marks each content link undelivered (`instances.rs`, where it reopens), but
  if the host attaches the editor again (`attaching`) it does not forward the app's `Resend`
  (`!(resend && attaching)`), and the replica's `attach` returns early for an editor it has
  already. Then nothing sends the snapshot, and what waits for the content (a keyboard gift's
  `Given` stage) waits for good. Done: a restarted app's editor is sent its snapshot in every
  case, shown by a be-host-runtime test that restarts an app with an editor attached and checks
  the content counts as delivered.

- **A folder's mirror once under load.**
  be-host-stack's `a_folder_whose_share_is_taken_back_leaves_the_mirror_with_what_is_in_it`
  failed once in 1010 runs (202 tests five times, 12 at a time) and passed 40
  of 40 alone. Done: the cause found, from a rerun under the same load that
  keeps its output, and fixed at its source.
- **A paired device's tree item that never came.**
  `an_offline_device_says_a_block_it_never_opened_is_not_on_it_yet` timed out
  once under verify waiting 30 s for `TreeItem "Garden"`. The server's late
  `tree-advanced` (a change told after the follower read it, which left the
  tree untold for the rest of the connection) could cause it, but the wait
  never failed in 300 runs under `stress` (`--parallel 4 --latency 75`), before
  that fix or after it, so the fix is not credited with it. Done: a failure of
  that wait found under `stress` and its cause fixed at its source.
- **A text typed into right after it is made.** The same test times out at its
  `type 'Plant the tomatoes'` with "a block type module has yet to settle",
  1 to 2 runs in 100 under `stress` at `--latency 75`. Done: the module that
  never settles found and fixed at its source, and the test passes 300 runs at
  that latency.

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
- **A drag threshold for every `on_drag`.** The native core calls `on_drag`
  on any movement of a press; `Interactive`'s `drag` waits for
  `DRAG_THRESHOLD` (or `TOUCH_DRAG_THRESHOLD`), but a plain `on_drag` user
  (a slider, a pan, the dock's grips) applies its own or none. Done: the core
  starts a press's drag at the threshold, and the components' own checks go.
- **A carry's first place in a window the host opens.** The board learns each
  place's window from the core's `Order` at the first point it tracks after a lift,
  so the lift's own choice takes a place whose window it does not know yet to be
  the pointer's; a dock window's place whose rect, in its own window's points,
  holds the pointer can be chosen for that one frame. Done: the board knows a
  node's window at the lift (a `Space` that answers it on both cores), and a
  test on the fake `Space` lifts with a place in another window under the
  pointer's coordinates.
- **Keyboard steps into a custom site.** A `custom` site (the video timeline)
  gives no fixed places, so a keyboard carry cannot step into it. Done: a
  custom site offers stepping points (a frame grid along the timeline), shown
  by a test that carries a clip there from the keyboard.
- **A keyboard step reveals the place it chose.** Edge scrolling follows only a
  pointer, so a keyboard carry stepped to a place in the hidden part of a scroll
  leaves it out of sight. Done: after each keyboard step the board scrolls the
  chosen rest into view in every scroll around it, as focus does.
- **A fixed list's carried row is listed twice.** A `fixed` `Reorder` keeps
  its own row's slot open at opacity 0 while the list draws the row's preview
  at its root, and an opacity-0 frame stays in the accessibility tree, so a
  screen reader finds the row twice. Nothing on either core hides a subtree
  from accessibility (aria-hidden on HTML). Done: the hidden row leaves the
  tree on both cores, shown by a drive step on a fixed list that finds the
  carried row once.
- **A floating row's announcements.** The live region that says a carry's
  places (`Interactive`'s `CarryStatus`) is inside the source, so a `Reorder`
  row carried off its list, collapsed, says nothing more until it lands, when
  only the receiving list's `ReorderStatus` speaks. Done: the places of a carry
  whose source is collapsed or gone are still announced, on both cores.
- **An embed moved by a removal a timer makes.** An `Embed` whose old parent is
  removed during a frame's input, and placed again by its frame's layout, is
  never heard unplaced (`EmbedState::hold`); one whose old parent goes in a
  later layout pass of the frame (a removal a timer due in that frame makes)
  is heard unplaced once, and its app hides and presents again. Done: such an
  embed is told only where the frame leaves it, shown by a test that moves a
  kept `Embed` from a timer.
- **A first measurement that is there at once.** On the HTML core a node's
  `measure` signal is `None` until something has asked for it and a frame has
  passed, so whatever a lift reads with `get_untracked` must be measured from
  when it is built (the dock's tiles, tab strips and held panels, a `Reorder`'s root). Done:
  beui-core-html's `watch()` seeds the signal synchronously the first time it
  is made, and those eager `measure` calls at build go.

- **The game's stage wraps its table.** The stage lays its pressing
  `Interactive` over the table because a drag source's face once took its
  child's own size; it lays its child out as a plain `Interactive` does now.
  Done: the `Interactive` wraps the table, and the game's paintings and press
  tests are unchanged.
- **Selecting a game piece from the keyboard.** Space lifts the selected
  piece and the arrows step it between its squares, but only a click selects
  one, so a game cannot be played from the keyboard alone. Done: the arrows
  move a selection between the movable pieces while nothing is carried, shown
  by a test that plays a chess move with keys only.
- **A sheet's detents from the keyboard.** A sheet chooses its detent with
  `closest(detents).thrown()` only where its drag lets go, so no key steps it
  between detents. Done: the sheet's handle steps between its detents with the
  arrows, shown by a test.
- **The display arrangement refits for a carried display.** The area's view
  fits the arrangement at the lift and clips what it draws, so a display
  carried to a place beyond that fit (below a display beside it) is cut off at
  the area's edge. Done: the area refits to the arrangement the chosen place
  makes, with places read relative to the refit, shown by a painting of a
  display carried below another.

## 18. Keys

- **A new phrase grows with the account.** `set-recovery-key` carries one
  retirement (about 300 to 400 bytes) per access log the account signed in,
  which is every block it ever made, beside the resealed keys that grow the
  same way, and `get-recovery-key` decodes each of those logs to serve its
  head. Past roughly 20,000 blocks a new phrase no longer fits in one frame
  (`MAX_FRAME_BYTES`), which locks a heavy user out of the one answer to a
  stolen phrase. Either send the change in parts committed by a last call
  that keeps the all-or-nothing check, or keep one account-level keys log
  holding the retirement once, which a block's check looks the signer's
  current key up in. Done: an account with more blocks than one frame's worth
  of retirements takes a new phrase, and every log is retired.

- **A key change sealed to a phrase its reader replaced.** A device follows its
  contacts' waiting key changes just before this account takes a new phrase
  (`accounts::set_recovery_key`), so the changes it sends seal to each
  contact's newest key. A contact that takes a new phrase between that read and
  the write, or before it follows this account's change, seals its change to
  this account's old key, which the new phrase cannot open: this account's
  devices find the chain broken and show the contact as not confirmed until a
  new link. Done: a key change names the recipient key it is sealed to, and a
  device holding the old secret (or the reseal, which could carry an opened
  copy) follows it, with a test where both accounts take new phrases at once.

- **A test server that lies in its answers.** Fixes for what "the server
  alone" can do (a block's parent, its current key, its history, a rollback)
  want tests that fail before the fix, but the server answers summaries and
  listings from its in-memory graph, which a test cannot change by editing the
  database, and the fault relay only stalls and cuts.
  `a_key_the_server_wraps_under_the_settings_key_and_names_current_seals_nothing`
  injects the summary client-side instead, so it passes on the code it fixed.
  Done: a test hook (in the harness's server, or the fault relay) rewrites a
  block's summary or listing (parent, current key, history) on its way to one
  client, and that test uses it and fails without the fix.

- **Commits sealed before a rotation.** A commit made offline is sealed under
  the key the device held when it wrote it. If the block's keys rotate
  meanwhile (a member removed), the drain publishes it under the old key, which
  the removed member still holds: neither `drain_publish` nor the server checks
  the key's epoch. Offline writes report `Saved::Kept`, version control's
  included, so this is the usual path. Done: the drain reseals such a commit
  (and those built on it) under the current key before sending it, or the
  server refuses one sealed under a superseded key (guides/blocks/keys.md,
  "Epochs name their heads"), shown by a client test where a member is removed while another
  device writes offline and the removed member cannot open what it sends.

- **A link's record below its own device's pin.** A record a contact link
  makes follows the newest the server holds for that contact
  (`contacts::served_verified`), not this device's own pin, so a server that
  hides the newest record from the accepting device makes that device list its
  own new link as an older key (flagged, never silent). Done: `accept_link`
  and `finish_links` also start after the caller's pin, shown by a client test
  where the server serves the accepting device an old record.
- **A revocation the block's log does not record.** Revoking someone the
  block's log names nowhere (a contact link's grant, applied by the server
  when the link was accepted and not yet finished by its maker's device) signs
  no record (`signed_grant` answers none), so only the server's row says the
  grant is gone: `Peer::finish_link` checks that row before signing the link's
  grant. Done: a revocation signs a `Level::None` record whenever the server
  holds a row for the grantee, whether the log names them or not, and
  `finish_link` reads only the log, shown by a client test that revokes a link's
  grant before it is finished.
- **A contact's name as they rename.** A contact's name and email are the
  card's from when the two connected (guides/blocks/keys.md, Contacts), so a
  contact who renames keeps the old name on their contacts' devices until they
  connect again. A name is changed in the server's portal, where no device key
  can sign it. Done: each account's devices publish a card signed with its
  identity key whenever its name changes (noticed from the account's row), the
  server keeps the newest and lists it with the contact, and a device shows the
  newest card that its pinned key signs, shown by a client test where a
  contact renames and the server's forged rename is refused.
- **Invitations that end only on the server.** An invitation that expires, or
  is accepted, leaves its record in the organization's log, and one that
  expires stays open there for good (expiry is the server's word), so the log,
  and every proof path of it served below Edit, which keeps invitation records
  whole, grows with every invitation made. An administrator's `tend` could end
  each expired one with an invitation record (`open: false`), and proof paths
  could then reduce invitation records that ended before the reader's own
  member record. Done when an organization that made many invitations serves a
  below-Edit member a path no longer than its live invitations need.

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

## 21. Component macro

- **Internal trait names in a prop's error notes.** A value a children slot or
  an exact prop does not take is reported with the right headline and label,
  but rustc's "required for ..." notes under it name `Exact`, `IntoExactProp`,
  `TakesChildren` and `ChildShape` (`a_frame_takes_one_child`,
  `a_loading_hands_its_content_no_value` in compile_fail/beui). Done: those
  notes name only the declared prop type and the slot's own trait, or are
  suppressed, shown by the compile_fail pins' full text.
- **Children written as an array.** `Children<T>`'s `Takes` has no impl for
  `[ChildSegment<T>; N]`, which the old `Into<Children>` setter took through
  `From`, so a hand-written `children={[..]}` fails (`view!` never writes one).
  Done: `Children<T>` takes an array of segments again, shown by a beui-tree
  test, or the guide says to build `Children` from the array.

## 22. Headless runtime

- **be-host-runtime without its gpu and presenter.** `be` links
  be-host-runtime to run host components headless (plans/headless_apps.md),
  which brings beui-documents, wgpu and the font stack into its binary though
  it never starts them. Done when `be`'s build time or size hurts: the runtime
  split so `be` links no gpu or presenter crate, which also cuts a caching
  edge between them and `be`.
- **No audio in headless `be`.** be-host-runtime's apps' audio (`audio.rs`)
  links rodio, and through cpal ALSA, so `be` and its tests need
  libasound.so.2 at load time (be-tool-cli's BUCK sets `LD_LIBRARY_PATH` to
  the sysroot for its test) though a host component plays nothing. A cargo
  feature cannot split it, since buck builds one be-host-runtime per platform
  for every crate. Done when `be` runs on a machine with no ALSA: audio behind
  a crate edge `be` does not take, or loaded at run time on first play.

## 23. Sandbox

- **A stopped wire keeps saying why.** An app's sandbox is dropped with it (`Wire::stop_sandbox`),
  and a sandbox call that comes later finds no server and is refused as unsupported, which reads
  as "this host runs no workers". Done when the wire keeps a `stopped` reason once it stops its
  sandbox and answers every later sandbox call with it, with a be-host-runtime test that calls
  after a stop.
- **Server::stop answers pending compiles.** `be_wasm_sandbox_host::Server::stop` answers each
  instance's waiting `next` but drops the compiles still under way, so their callers are never
  answered. Done when it answers each with the stop's reason too, with a test that stops a
  server mid-compile.
- **The PDF renderer keeps its document.** Every render job carries the whole PDF
  (be-worker-pdf's `Job`), so the editor copies it into the renderer and PDFium loads it again
  for each tile. Done when the renderer keeps the last document it opened, by the block's
  revision, behind an Open and a Render message, and a job sends the bytes only when the revision
  is new, with a be-worker-pdf-render test that renders twice from one Open.

## 24. Build tooling

- **Restarting buck2's daemon after a lost blob.** buck2 keeps a 12-hour record
  of what `FindMissingBlobs` answered (its gRPC client's `FindMissingCache`,
  which no buckconfig option bounds) and never forgets an entry when an
  `Execute` is refused, so after the server loses a blob every retry against
  the same daemon fails with "Action ... is missing from CAS" until someone runs
  `./scripts/buck kill` (guides/build_server.md). re-relay already forgets such
  blobs. Done: when an attempt fails that way, `retrying` in
  crates/be-tool-bootstrap/src/buck.rs runs `buck2 kill` before the next
  attempt, and the guide's first step goes. First check that buck2 exits 2
  (which `retrying` retries) rather than 3 on an `Execute` FAILED_PRECONDITION,
  and that the failure can be seen: the output is not captured when the
  command is not quiet, so the wrapper may need another sign, such as a
  `forgot` line re-relay wrote to `target/re-relay.log` during the attempt.
