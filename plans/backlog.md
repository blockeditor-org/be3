# Backlog

Depends on: none.

The running to-do list, grouped. Every item says what "done" looks like.
Fixed items are removed as they land.

## 1. Text

- **An unquoted list code block's continuation spaces.** A fenced code block in
  a list outside a quote draws the list's continuation spaces as code text, so
  its content sits about 8px right of its own fence (the fence's backticks at
  x≈87, the code at x≈95 in be-app-text's markdown showcase, lines ~187-189),
  where a quoted one now keeps them in its marker, unseen, and lays the nesting
  as a gap (`quote_prefix`). Done: an unquoted list code line keeps the
  continuation spaces in its marker the same way, so its code lines up with its
  fence, shown by the showcase painting.

## 2. Docking

- **A bar on a deeper layer's pane, saved.** A joined dock's bar dropped on a
  pane of a dock joined below its own layer goes back to its joined dock's
  first pane after a save and restore: `DockingLayout::snapshot` unhosts the
  joined groups first, and `extract`/`graft` (beui-dock-state `joined.rs`) map
  a bar's `pane` through its own layer's tabs, so a deeper tab saves as `None`.
  Bars from several layers on one pane also come back outer layers first
  (`layer_place`), not in the order dropped. Needs a decision on the saved
  form (a path through the hosting tab, or each bar's slot across layers).
  Done: both survive a save and restore, shown by a beui-host dock test.

## 3. beui platforms

- **A live resize on macOS.** `CAMetalLayer`'s default `contentsGravity` (resize) may stretch
  the last frame while a window is resized live, before beui draws the new size. Check it, and
  set it to top left if so, as X11 keeps the last frame at the corner (beui-platform-winit's
  `backdrop.rs`). Done when a live resize on a Mac shows the last frame unstretched.

## 7. Sessions

- **A Join channel test that once heard more than Closed.** be-host-stack's
  `a_joined_channel_opens_to_its_host_and_again_to_the_next` once failed where the first
  host leaves: it heard something other than (or before) `Closed`, maybe a second `Closed`
  or an `Open` from a handshake racing the leave. It now waits for the server's word that
  the lease moved and prints the events it heard. Done: the next failure's events read,
  and the ordering they show fixed or explained.

- **Hosted channel races without tests.** Three fixes in hosted blocks 4b
  close races that land between two awaits, which no test forces yet: an
  admit a recheck ran beside asks for another check (`Member::Answering`'s
  `admit`, `Hosting::snapshots`), a recheck drops a member only while it speaks
  with the account and key it checked (`hosting::drop_checked`), both in
  be-host-stack's hosting.rs; and be-server's participant join re-checks the
  ban after registering (`Client::join`). Done: a test-only pause between the
  two awaits of each, and a test that changes access or bans in it.

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

- **A lost seal's rebase with a merge above it.** Before leases came from
  the session order, the server could lose a seal's
  `rebase-journal` while the outbox published the seal and seals made on it
  since; `journal_for` finds the seal on the server journal's base by walking
  down the head's single-parent seals. A merge commit between them, or more
  than `SEAL_WALK` offline seals above it, stops the walk, so the replica opens
  on the journal's base and merges the head in on top of entries the head
  already holds, counting them twice. Leases from the order closed the cause: a seal the outbox
  publishes carries its journal seal and trims the server's journal as it wins
  the head (be-host-stack's
  `a_seal_whose_publish_was_lost_trims_the_journal_when_the_outbox_drains_it`).
  The walk stays until no path is shown to reach it: a seal that wins the head
  untrimmed (a stale seal, one the host made in a made-up session) with entries
  past the journal's base it holds. Done: either such a path and a test that
  counts exactly what was made through it, or the walk removed with a test that
  shows every seal on the journal's base trims it.

- **A terminal host stands by on a timer.** Since a claim takes the lease at
  once, a second terminal host waits `CLAIM_INTERVAL` (3 s) after it first sees
  a new holder before claiming, so the holder's presence can reach it first
  (crates/be-tool-cli terminal_host.rs). Under real load that presence can take
  longer, and the second host then steals the lease and the viewer's keys are
  lost. Replicas already send their presence to each joiner, so stand by until
  the holder's presence arrives or `CLAIM_INTERVAL` passes: at once in the
  common case, the timer only for a holder that sends no presence. Done: a
  test where the holder's presence is held back past the join, and the second
  host still stands by once it arrives. What it needs first: presence reaches the host only
  through its replica's `EditorMirror` (be-tool-cli's replica.rs), which lists the peers
  that show something, not which peers it has heard from at all, so "a holder that sends
  no presence" cannot be told from one whose presence is late until the replica says when a
  peer's presence (empty included) has arrived.

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
  its source, with a test that shows it. Not seen since the take ordering fix
  (`telling` in be-host-stack's lib.rs), which may be what it was: 0 of 600 runs
  alone (200 at 8 at a time, 400 at 16, on a 4-core VM, the harness run
  directly) and 8 of 8 runs of the whole test binary, 4 at a time.

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
  source, with a test that shows it. The suspect is gone: a text that could not be
  read is no longer kept (`text_of` answers `Err`), though the hit shown then still
  has no snippet until its answer is shown again, when the mirror next moves.

- **A crash test that once never counted 11 after the restart.**
  `an_edit_to_a_block_made_editable_again_after_its_edits_were_discarded_lands_once_across_a_crash`
  (be-host-stack) hung once in 2000 local runs (16 at a time, the harness built once and run
  directly) at the `wait_for_count(block, 11)` after its restart, with nothing owed, undurable or
  draining. Whether it counted 1 (the edit lost) or 21 (ordered twice) was not recorded;
  `wait_for_count` now says what it last counted. Done: the cause found from that output and
  fixed at its source, with a test that shows it.
- **A restarted app's editor that is never sent its snapshot.** When an app restarts,
  be-host-runtime marks each content link undelivered (`instances.rs`, where it reopens), but
  if the host attaches the editor again (`attaching`) it does not forward the app's `Resend`
  (`!(resend && attaching)`), and the replica's `attach` returns early for an editor it has
  already. Then nothing sends the snapshot, and what waits for the content (a keyboard gift's
  `Given` stage) waits for good. Done: a restarted app's editor is sent its snapshot in every
  case, shown by a be-host-runtime test that restarts an app with an editor attached and checks
  the content counts as delivered. Not found yet: `Instances::reopen` keeps each link's
  `attached`, so a restarted app's `Resend` is forwarded and the replica sends the snapshot
  (`a_restarted_app_is_sent_its_content_and_blocks_again`), and a link that goes calls
  `detach`, which takes the editor off the replica too, so a later `attaching` meets a replica
  without it, whose `attach` sends the snapshot. What leaves a link unattached while the
  replica still holds its editor is the thing to find first.

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
  that latency. Not seen in 100 runs at 8 at a time and
  `--latency 75` on a 4-core VM after be-host-runtime counted an app starting for a
  view the host asked of it as busy.
- **The search index's test connecting, once under verify.** be-host-stack's
  `what_is_typed_is_found_through_the_workspaces_search_index` failed once in a
  full `./scripts/verify` after 304 s, connected: false and "connecting to the
  server timed out", waiting to reconnect; it passed alone in 2 s and in the
  verify rerun. Done: the cause found (the test's server under the build
  server's load, or a reconnect that never comes back) from a failing run's
  output and fixed at its source, with a test that shows it.

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
  clips (guides/beui/components.md, Carries and drop sites). Done: the source's face
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

- **A lift wakes only the carries it concerns.** The board keeps every carried
  payload in one `lifted` signal, so each `carries(&payload)` reader (every
  slider, colour control and scrollbar thumb's `ThumbCarry`, every splitter, every
  `is_carried` row) reruns on any lift or put-down anywhere in the tree. Done: a
  per-payload signal or keyed subscription, so a lift wakes only the readers of
  its own payload, shown by a test that counts the effects a lift runs.
- **A game item finds its place in one step.** Each `PieceStop` (be-app-deterministic-game
  board.rs) finds its item with `Layout::item(id)`, a scan of the layout's items, so a new
  layout costs every item a scan. Done: the layout keeps an index from `ItemId` to its
  place, built once with it, and `item` reads that.
- **The display arrangement refits for a carried display.** The area's view
  fits the arrangement at the lift and clips what it draws, so a display
  carried to a place beyond that fit (below a display beside it) is cut off at
  the area's edge. Done: the area refits to the arrangement the chosen place
  makes, with places read relative to the refit, shown by a painting of a
  display carried below another.

## 18. Keys

- **An agent follows no key change sealed to a key it replaced.** An account
  keeps each identity secret a new phrase replaced in its own block
  (`AccountBlock::retired`), so a contact's key change sealed to the old key
  still opens; an agent has no such block, so a change its maker seals to the
  agent's old key, when the two take new phrases at once, breaks the chain until
  the maker makes the agent again. Done: an agent keeps its retired secrets
  where its maker's devices and its own read them, with a test where an agent
  and its maker take new phrases at once.

- **Keys a contact seals to a key the reader replaced.** A key a contact seals
  to this account while it still holds the account's old key (`reseal_from`,
  `ResealKeys`, and every seal between the two moves) opens only with the
  current secret, so it stays sealed until the contact follows the change and
  seals it again. Done: those opens try the retired secrets too
  (`contacts::Secrets`), with a test where a contact seals a key to the old key
  after the account's move.

- **An app opens only a block the server confirmed it made.** An app whose
  manifest does not set `sharing` may open a block it made in this run to
  anyone inside (`refuses_sharing`), judged by the local graph, before the
  server has the create. An app that names, as its new block's id, a block the
  person manages that this device has not loaded opens that block in the window
  before the server refuses the create. Done: the host hears when the server
  takes or refuses a create, and the exception holds only for one it took, with
  a test where the create names a block that already exists.

- **A contact's name rolled back.** A device shows a contact by the name card
  the server serves that the contact's key signs (guides/blocks/keys.md,
  Contacts), so a server can serve an older card the contact signed and show
  the name they replaced. Done: each device keeps the highest card date it
  showed per contact in its refs and shows no card dated before it, with a
  client test where a lying server serves the older card.

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

- **`./scripts/test NAME` for a crate whose tests are not `:test`.** When NAME
  matches a test in crates/beui, the script asks for `//crates/beui:test`, which
  does not exist (its only test target is `:html-test`), and reports a build
  failure beside the tests it ran (`./scripts/test scrollbar`). Done: the script
  runs each crate's real test targets, so naming one of crates/beui's tests runs
  `:html-test` and nothing fails to build.

- **Restarting buck2's daemon after a lost blob.** buck2 keeps a 12-hour record
  of what `FindMissingBlobs` answered (its gRPC client's `FindMissingCache`,
  which no buckconfig option bounds) and never forgets an entry when an
  `Execute` is refused, so after the server loses a blob every retry against
  the same daemon fails with "Action ... is missing from CAS" until someone runs
  `./scripts/buck kill` (guides/build_server.md). re-relay already forgets such
  blobs. buck2's restarter (`[buck2] restarter`, on in .buckconfig) does not
  help: it restarts only on errors buck2 marks as corrupting its state, and it
  counts an `Execute` FAILED_PRECONDITION as an ordinary failure. Fixing that
  in buck2 (mark the refusal as corrupt state, or forget the named digests in
  `FindMissingCache`) would replace this item. Done: when an attempt fails that way, `retrying` in
  crates/be-tool-bootstrap/src/buck.rs runs `buck2 kill` before the next
  attempt, and the guide's first step goes. First check that buck2 exits 2
  (which `retrying` retries) rather than 3 on an `Execute` FAILED_PRECONDITION,
  and that the failure can be seen: the output is not captured when the
  command is not quiet, so the wrapper may need another sign, such as a
  `forgot` line re-relay wrote to `target/re-relay.log` during the attempt.

## 25. Tentative gestures

What the tentative layer (guides/blocks/host.md, Undo;
decisions/tentative_edits_wait_for_release.md) left as it was.

- **A held layer's ordinary edits rebuild every view.** An ordinary edit made
  while a tentative layer is held rebuilds `visible` and snapshots every editor,
  per keystroke, and since `unsettled` counts the layer, a peer's entry while it
  is held may rebuild `visible` and snapshot the editors rather than land beneath
  it. `Journaled::Beneath` with the layer as `local` would tell them less. Done:
  both go beneath the layer, with a be-sdk-type-guest test counting that typing
  during a held drag sends editors no snapshot.
- **A gesture's dropped edits answered by a snapshot each.** An edit the
  replica drops in a gesture it ended (an undo mid-drag, a replacement), sent
  before the editor heard, is answered with a snapshot, once per frame still in
  flight; it did not show. Done, if drags send enough frames in flight for it
  to matter: `EditorMirror` tags its pending edits with their gesture and drops
  them itself on `GestureEnded`, with a be-sdk test that an undo mid-drag sends
  one snapshot at most.
- **Two `LiveContent`s for one block.** `EditorHost::content_of` makes a new
  `LiveContent` per call, and two live at once for one block in one instance
  share its queue of content updates (the first to adopt takes them all,
  `GestureEnded` included). Done: `content_of` goes through the editor's
  projection cache, or asserts one projection per block, with a test.
- **Undo groups left open by an editor that detaches without a goodbye.** An
  editor that detaches without the runtime's cancels for its open gestures
  leaves its undo groups open in `History` until an undo or redo commits them
  (`close_groups`); its tentative gestures are cancelled by the detach itself.
  Done: the detach commits its undo groups too, with a be-sdk-type-guest test.
- **Undo while a voided gesture is open.** After an undo voids the open
  tentative gestures, a second undo is swallowed and redo refused until their
  editors end them, alike on host and replica; and a gesture the replica voids
  itself (a replacement, an edit that does not carry) that its editor commits
  before hearing `GestureEnded` counts its operations though they were dropped.
  Done: decided whether either needs more than the editor's prompt cancel, and
  if so a be-sdk-type-guest test of it.
- **Peers' derived views of a held edit.** Peers see a tentative edit only
  through presence (the drag board's frames), so their derived views (bound
  lines, z-order) do not follow it. Done: presence carries the held edits and a
  peer's replica previews them as a layer of its own, shown by a canvas test
  with a bound line.

## 26. Server protocol

- **Access logs checked once, what is left.** A device checks each entry of
  a whole log once and reads only what was appended, and the server checks an
  append alone, but: a device's first read after a restart is whole and
  hashed against `KeptLog::checked` (keeping the entries with the refs would
  drop that); authority from another log (`be_block_keys::answer`) checks that
  log from its root as of the entry a record names, each time (keeping states
  as of the heads records name would drop that); a hosted block's log checked
  from an unpinned root (`joined_log`) keeps nothing; the server still loads
  and hashes every entry of a log on each check (keeping the head's place and
  hash in the database would drop that); and `Checked::positions` grows by an
  entry's hash for each entry, since epochs name their heads by hash. Done:
  each with a test that counts the work it drops, as be-server
  `an_append_checks_only_the_appended_entry_on_the_server` does.

- **A block known only through a reference.** A block a reader knows only
  exists (through a reference) names no entry point, so it is read again only on
  `roots` or a reconnect (decisions/server_protocol_is_five_calls.md). Done:
  such a block is told when it changes, with a be-block-client test where a
  reader that knows a block only through a reference sees its new summary
  without reconnecting.

- **A deadline on writing an answer.** hyper writes an answer for as long as
  the client takes to read it, so a client that never reads a large S3 object
  (up to `MAX_OBJECT_BYTES`) or a long-poll answer holds its connection and
  place until the client goes. Done: `http::serve` gives up on a connection
  whose answer has made no progress for `IDLE_TIMEOUT`, with a be-server test
  where a client that stops reading a GetObject answer is disconnected.

- **A Backrefs answer that leaves out a block just made.** On the web, after a
  text is created, be-host logs a few times "the answer to Backrefs(...) at
  sequence N left out [...], which this device holds as listed there"
  (`log_unlisted_held` in be-host-stack's `worker.rs`, from
  `Graph::take_unlisted_held`). Repro: `./scripts/test
  //crates/be-host:shown-again-web-host-test`, which makes two texts, and read
  its log for those lines (the `stale` field says whether the answer was to a
  request from before the change). Whether it is a request answered from
  before the new block's reference was published, or the server's backrefs
  lagging its listing, is not known. Done: the cause is found, and a
  be-host-stack test that makes a block and lists its backrefs right away sees
  no such line.

- **One bad log entry refuses its whole batch.** `Peer::send_appends` sends
  a block's waiting entries in batches (`ENTRIES_PER_SEND`), and the server
  refuses an append whole, so one entry it would refuse (over a limit, a
  redaction it will not take) refuses every entry sent with it, each then kept
  as refused (`Drained::NotAppended`). Done: the entries a refused batch held
  go again one at a time, or the server names the entry it refused and takes
  the rest, with a be-block-client test where one bad entry among good ones
  leaves only it refused.

## 27. Unread counts

- **An open block counted live.** A block's unread count is counted from its
  head (`crates/be-host-stack/src/unread.rs`), so a chat open elsewhere (a
  background tab) counts a new message only once its session seals it, a few
  seconds later. Done: while a replica of the block is open, the count comes
  from the replica's confirmed content as operations land (throttled, one
  module call per burst), falling back to the head when it closes, with a
  be-host-stack test that sees a message relayed in an open session count
  before any seal.
- **Badges beyond the files pane.** Only file-tree rows show `CountBadge`.
  Done: the block picker's rows and an editor tab's title carry the block's
  `BlockInfo::unread` the same way, each with a painting.
- **Unread rows in bold.** Done: a file-tree row whose block has an unread
  count shows its name bold, as a messaging app's conversation list does, with
  the file tree's painting showing it.

## 28. Member names

- **A member removed from the organization leaves the names.** An editor's
  members (`block-subject.members`) are asked for again when the block's
  `changed` moves (be-host-runtime `members.rs`). Removing a member from the
  organization revokes only the grants they held (be-server `remove_from`), so
  a block they viewed without one of its own (one they authored, or one open
  in a workspace they viewed inside) keeps its `changed`, and an open editor
  names them until the block next changes. Done: a member's removal reaches
  the editors of every block they could view (the server moving those blocks'
  `changed`, or the stack asking again as the organization's members change),
  with a be-host-runtime test where removing the author of a block shared with
  this account drops them from an open editor's members.
- **A revoke then a publish stalls the flush.** While writing be-host-runtime's
  `an_editor_hears_a_member_go_when_their_access_is_revoked_with_a_publish`, a
  variant that revoked a block agent's access to an open block and then
  published to it hung in `flush_and_send` until its 60 s limit; publishing
  first and then revoking (the committed test) does not. Not looked into: it may
  be the reseal under the new key racing the publish, or the test's wait.
  Done: the cause is found and fixed or explained, with a be-host-runtime or
  be-host-stack test that revokes and then publishes on an open block and sees
  the publish land without waiting out a limit.
