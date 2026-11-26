# Backlog

Depends on: none.

The running to-do list, grouped. Every item says what "done" looks like.
Fixed items are removed as they land.

## 3. beui platforms

- **A live resize on macOS.** `CAMetalLayer`'s default `contentsGravity` (resize) may stretch
  the last frame while a window is resized live, before beui draws the new size. Check it, and
  set it to top left if so, as X11 keeps the last frame at the corner (beui-platform-winit's
  `backdrop.rs`). Done when a live resize on a Mac shows the last frame unstretched.
  Checked: wgpu 30 makes its layer through raw-window-metal 1.1, a sublayer that leaves
  `contentsGravity` at `kCAGravityResize` (its `observer.rs`), so it does stretch. Setting
  `kCAGravityTopLeft` on it, as `Surface::as_hal::<Metal>` gives its `render_layer()`, after each
  `create_surface` (beui-renderer-wgpu `window.rs`, `present.rs`), is the likely fix; left for a
  session with a Mac, since nothing here builds or runs macOS.

## 7. Sessions

- **An admit a recheck ran beside, without a test.** An admit a recheck ran
  beside asks for another check (`Member::Answering`'s `admit`,
  `Hosting::snapshots`, be-host-stack's hosting.rs), which no test forces: every
  revocation a test can make there (a lowered grant, a ban) also has the server
  take the member out of the session, and the host drops it on hearing that
  whether or not it checks again. Done: a test held at `hosting::ANSWER_PAUSE`
  with a change the signed logs refuse and the server does not act on (a new
  identity key for the account), or with the server's word of the leave held
  back.

- **A seal of its own during a join that holds kept edits.** `rejoined` seals
  and waits only for a join begun on a settled made-up session
  (`Open::joined_settled`). One that joins holding kept or doubtful edits takes
  the server's journal as it is; if its made-up session sealed on its own while
  the join was on its way (its seal timer, a close), that commit is pending in
  the outbox and the replica reopens beside it, as the settled case did before
  `sealed_joining`. It is expected to resolve once the drain sends the commit
  and the replica merges it, with the sealed edits given up and not given again.
  Done: a be-host-stack test that reopens a block after a crash, has its
  made-up session seal while the join is held (`be-lib-test-pause` can hold
  it), and counts exactly what was made.

- **A kept submission carried over a restart that does not encode again.** A
  replica opened again submits the edits a kept submission holds under its id
  only if they go as its payload did (`Replica::flush`: the same inline bytes,
  or the same part, which putting the same bytes under the same key makes
  again); otherwise they go as a new submission, and if the first copy reaches
  the server late, from the process that crashed, the order takes them twice.
  An edit carried over a restart of the order goes again from `replay` as typed
  while its kept submission holds the operation carried over, which differs for
  a type whose `carry_over` rewrites it (text's positions). Done: such a
  submission matched by the edits it was carried from and sent again under its
  id, with a guest test where its first copy lands after the one given again.

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

## 12. Large folders

- **A listing read again is applied whole in memory.** A listing now writes only the
  blocks that joined or left it (`Table::ListingEntry`), but each `Refs::listed` call
  still lays the outbox over every summary and compares the whole id set, so a watcher
  that reads a folder again as each of N children joins spends O(N²) CPU (be-block-client's
  `a_watched_folder_gaining_thousands_of_children_writes_in_proportion_to_them` spends
  seconds on 2000). Done: when measured to hurt, a listing read again applies only what
  changed since the last (a page of the tree's changes, say), with a test bounding the
  work per child.

## 15. Flaky host tests

- **A seal's lost publish and a joined channel, often at 16 at once.**
  Run straight from be-host-stack's built harness, each in its own temp dir, 16
  at once on 4 cores, `a_seal_whose_publish_was_lost_trims_the_journal_when_the_outbox_drains_it`
  failed 22 of 240 ("the stack never reached counted 5: it made no progress for
  60 s") and `a_joined_channel_opens_to_its_host_and_again_to_the_next` 95 of
  240 ("the channel to the first host closed: []"). Done: each cause found and
  fixed at its source, and each passes 400 runs at 16 at once. Neither failed on
  568d72d49 (2026-11-25): 0 of 1000 each at 32 at once on 4 cores, and 0 of 240 each at 16 at
  once beside two whole-crate runs, nor did `an_edit_sealed_offline_onto_a_seal_the_server_lost_lands_once`
  (0 of 600 at 32), so what failed is not reproduced locally: the next failure's output (a full
  verify keeps it) is what to read. Nor on b46be69e6 (2026-11-26): 0 of 460 each, up to 72 at
  once on 4 cores, and none of them in three runs of the whole crate, each test its own
  process, 32 at once.

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
  a 4-core VM (8 at a time, with every app and with `--apart`, and
  `--latency 60`). Done: run it under `stress` until it fails, and fix the cause
  its output shows at its source, with a test that shows it.

- **Quick open's hit without its snippet, once under verify.**
  `quick_open_finds_a_word_typed_in_a_text_and_opens_it` timed out waiting for
  the snippet "ripen late" with its result row shown. It passed 460 of 460 under
  `stress` on a 4-core VM (8 at a time with every app, `--apart --latency 60`,
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
  already. Then nothing sends the snapshot, and what waits for the content waits for good. Done: a restarted app's editor is sent its snapshot in every
  case, shown by a be-host-runtime test that restarts an app with an editor attached and checks
  the content counts as delivered. Not found yet: `Instances::reopen` keeps each link's
  `attached`, so a restarted app's `Resend` is forwarded and the replica sends the snapshot
  (`a_restarted_app_is_sent_its_content_and_blocks_again`), and a link that goes calls
  `detach`, which takes the editor off the replica too, so a later `attaching` meets a replica
  without it, whose `attach` sends the snapshot. What leaves a link unattached while the
  replica still holds its editor is the thing to find first.

- **Edits typed after a demotion, once under load.**
  be-host-stack's `edits_typed_after_a_demotion_stay_discarded_once_the_block_is_editable_again`
  failed once in a whole-crate run (its `count_of` was not `Some(11)` after the flush, line 114)
  and passed 6 of 6 alone. Seen again later counting 21: the edit of 10 typed after the block
  was editable again counted twice, in a run where the replica was told of a head another
  device moved (`Replicas::moved`, not from this device) just before it reloaded. It did not
  fail in about 110 runs among whole-crate runs, 12 at a time on 4 cores. What was ruled out:
  the sharer's `grant` only appends a log record, so it moves no head; `moved` with
  `from_this_device` false means only that the server told the head (`moved_head`), which this
  device's own seal is too when the outbox, not the replica, published it. A count of 21 is
  what a three-way merge counts when its base is the head before the replica's own seal of the
  edit (1) while both sides hold the edit (11 each): `Then::FastForwardMerge` merges from
  `self.base`, so a head that descends from a seal of this session past the replica's base,
  but is not that seal (`sealed_here` answers `Resume::Sealed` only for the seal itself), would
  do it. Done: the cause found, from a rerun under the same load that keeps its output, and
  fixed at its source.
- **A root's Files child said to be opening, under load.**
  `be_root_in_place_of_the_hosts_ui_shows_its_signed_in_block_stack` paints "the block's editor
  is opening" over the Files panel in about 3 of 40 `stress --apart --parallel 8` runs, before
  and after the root's children's statuses were told again each pass (crates/be-host/src/root.rs,
  `serve_editors`). The host's own statuses for it go from opening to available (seen in a
  print), so the workspace app keeps or shows the old one: where a child's `error` is drawn, and
  whether a later status reaches it before the root says it owes nothing, is what to look at.
  `be_root_in_place_of_the_hosts_ui_shows_a_joined_dock` shows the same text over both its
  children, its joined dock never shown, in 4 of 30 runs on 7c707c052 and 3 of 30 after.
  Done: the cause found and fixed at its source.
- **A replica waiting for its head's objects, under whole-crate load.**
  be-host-stack's `a_block_whose_replica_waits_for_its_heads_objects_does_not_export` failed
  in 6 of 8 batches that ran the whole crate once among about 500 runs, 12 at a time on 4
  cores ("the export failed as not available yet", its line 67), and passes alone. Done: the
  cause found and fixed at its source.
- **A paired device that stays loading its files.**
  `keep_hosting_outlives_the_editor_until_stop` timed out in 3 of 200 `stress` runs (8 at a
  time) before any hosting: a device just paired (`Approve`) stays on "Loading the files" and
  never shows `#file-tree.new`. Seen again in 3 of 160 runs at 8 at a time on 4 cores (the rest
  of that run's failures were the kernel killing hosts for memory: two hosts a run is too many
  at 8 at a time on a 15 GB machine, so use `--parallel 4` there). In one (run 72) the step
  timed out with no busy reason and drive said the device was in step: the Files pane's
  `#editor.loading` stayed while nothing was pending, so the file tree's editor waits on
  something `busy` does not count (`awaiting_content` counts only a link `attached` and
  `may_load`; check which of the two is false for the file tree's block then). In the other two
  the paired device's app "did not answer within 35 seconds". Done: the cause found from those
  runs' logs and fixed at its source.
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
- **be-host-runtime's stopping apps, in parallel with other suites.**
  `a_panic_while_the_host_runs_an_app_stops_only_that_app` (its line 44) and
  `a_stopped_apps_view_keeps_the_place_it_is_shown_at` (line 27) each failed
  in targeted runs of several suites at once on the build server, and pass
  alone. One cause is fixed: with `RUST_BACKTRACE` set, each test's contained
  panic symbolized a backtrace of the whole debug harness (0.7 s, 300 MB), and
  96 at once failed 201 of 400 locally; a contained panic outside be-host now
  prints one line (`be_wasm_panic::contain`), 0 of 1000 after. The build server
  does not set `RUST_BACKTRACE`, so a failure there again has another cause.
  Done: if it recurs on the build server, what each waits on found from that
  run's output and made an explicit signal.
- **An edit waiting on a session whose lease holder switched organization.**
  Seen as "1 submitted, not yet numbered" on a view-state block (`editor-view`),
  in `players_carry_on_with_the_next_host_once_the_first_closes_the_world` and
  elsewhere, right after a device made or switched organization. One cause is
  fixed: an entry of the order sealed with a key this device did not hold yet
  was counted heard before it waited for its key, so it was passed over once the
  key arrived and the replica kept a gap that held back its own edit
  (guides/blocks/host.md, "A lost entry"; the be-block-type-host test
  `an_entry_sealed_with_a_key_not_yet_here_reaches_the_replica_once_it_arrives`).
  Two more that could hold back a session's events are fixed in
  `Connection::join`: a second join of a block on its way threw away what the
  first held and let its stale answer tell who hosted, and a join dropped
  unanswered held the block's events forever (the be-block-client tests
  `of_two_joins_of_one_block_on_their_way_only_the_newer_answers_for_the_session`
  and `a_join_dropped_unanswered_holds_none_of_its_sessions_events`).
  The host test itself never failed locally (0 of about 380 runs under
  `stress`, 50 to 300 ms of latency), so whether that was its only cause is not
  shown. Done: if it recurs, the failing run's host log read for a recorded
  error opening an entry, and that cause fixed at its source.

- **Keep hosting through a closed editor, under verify load.** be-host-tests'
  `keep_hosting_outlives_the_editor_until_stop` failed in 3 of 5 full verifies on one branch
  (and passed in 1 of 1 on its base, and alone every time), two ways: timed out at
  `wait not #dock.editors.menu.editor.keep-hosting` after reopening the closed Counter, and
  timed out busy, one publish of the Counter left in the outbox through every drain, the last
  `Diverged` (local and remote commits differ) with "object ... is nowhere to be found". Done:
  the cause found and fixed at its source, and the test passes 200 runs under `stress`. On
  568d72d49 it passed 220 of 220 (100 at 4 at a time, 120 at 6 with `--apart --latency 75`),
  and `players_carry_on_with_the_next_host_once_the_first_closes_the_world` 120 of 120 the
  second way. Ruled out for the first way: the phone claiming on a stale word that no one
  hosts as it reopens the Counter; with the phone's answers held from its close until after
  it reopens, it still stands by (its replica leaves the session as the tab closes). On
  b46be69e6 it passed 100 of 100 (40 at 4 at a time, 60 at 6 with `--apart --latency 75`), as
  did `players_carry_on_with_the_next_host_once_the_first_closes_the_world` and
  `typing_after_a_pause_undoes_as_its_own_step` (60 of 60 each, the same way), and
  `be_root_in_place_of_the_hosts_ui_shows_the_children_its_session_places`, `..._lands_a_block_dragged_from_the_files_on_a_canvas`
  and `..._shows_a_picker_a_canvas_places` (50 of 50 each, 6 at a time with `--apart`), which
  verify has seen fail too: their next failure's output is what to read.

## 17. Dragging

- **A carried item's own face, unclipped.** A carry with no `preview` floats as
  its source drawn displaced and raised, which a scroll around the source still
  clips (guides/beui/components.md, Carries and drop sites). Done: the source's face
  captured as a texture on both cores floats in a top layer like a preview,
  so no carry needs `preview` just to leave its scroll.
- **A floating row's announcements.** The live region that says a carry's
  places (`Interactive`'s `CarryStatus`) is inside the source, so a `Reorder`
  row carried off its list, collapsed, says nothing more until it lands, when
  only the receiving list's `ReorderStatus` speaks. Done: the places of a carry
  whose source is collapsed or gone are still announced, on both cores. Found
  since: a pointer carry says only its lift and its end (places are said only
  as the keyboard steps), and a keyboard carry off its list keeps its row (drawn
  at no size), so the gap is the end of a carry whose source goes with the move:
  Ctrl+Shift and an arrow moving a row into the next list (the `reorder_lists`
  fixture in beui-host's tests) says nothing at all, since the row that held the
  live region is gone and the receiving `Reorder` announces only its own rows
  (`commit` in reorder.rs). Needs a speaker that outlives the source (the
  board's `said` read by a region outside every source, or the receiving site
  announcing an arrival), which beui has no place for yet.
- **The display arrangement refits for a carried display.** The area's view
  fits the arrangement at the lift and clips what it draws, so a display
  carried to a place beyond that fit (below a display beside it) is cut off at
  the area's edge. Done: the area refits to the arrangement the chosen place
  makes, with places read relative to the refit, shown by a painting of a
  display carried below another. The places' rests are in area points of the view (`arrangement_spec`
  in be-app-settings display/arrangement.rs), so a refit that follows the
  chosen place moves the rests the choice is made among: the refit needs the
  rests in desktop pixels mapped through the view at each point (a `free`-like
  spec of its own), or a view that only grows while a carry lasts.

## 19. Local accounts

- **A server for a local account.** Done: a local account can be given a
  server, sending everything it holds there as a push to a new remote, and goes
  on as that server's account.

## 21. Component macro

- **Internal trait names left in a slot's error notes.** A children slot asks its
  type for the shape's trait directly, so `ChildShape` and `TakesChildren` no longer
  show in rustc's "required for ..." notes. Two hops still do: an `Option` slot's
  `TakesOptionalBlock` (`a_frame_takes_one_child`), and the `Exact<P>` or
  `ExactWithin<P>` wrapper of a slot whose type comes from what is passed
  (`a_loading_hands_its_content_no_value`), both in compile_fail/beui and pinned
  there with `//~ NOTE`. Done: those notes name only the declared prop type and the
  slot's own trait, or are suppressed, shown by the pins; left until rustc can hide
  a blanket impl's note while keeping the leaf trait's `on_unimplemented` headline.

## 22. Headless runtime

- **be-host-runtime without its gpu and presenter.** `be` links
  be-host-runtime to run host components headless (guides/apps/runtime.md, "Headless"),
  which brings beui-documents, wgpu and the font stack into its binary though
  it never starts them. Done when `be`'s build time or size hurts: the runtime
  split so `be` links no gpu or presenter crate, which also cuts a caching
  edge between them and `be`.
- **A deployed `be` running apps.** Reading manifests and type modules from a
  deployed `be`'s release (checked against the `app-hashes.json` its
  `build-info.json` pins) and `be BLOCK host` have run only against apps staged
  beside a `be` built here, never a real deploy: only releases deployed after
  them name their `apps`. Done when a deployed `be` has read a block type and
  hosted a block from its release, or a test serves a release as the mirror does.

## 24. Build tooling

- **CI still installs Python Playwright for old pull requests.** ci.yml's
  "Install the browser preview runtime" step serves only checkouts from before
  the browser rasterizer brought its own Chromium (those with
  crates/be-tool-browser-rasterizer/requirements.txt); a newer checkout skips
  it. Done: no open pull request has that file, and the step is deleted.

## 25. Tentative gestures

What the tentative layer (guides/blocks/host.md, Undo;
decisions/tentative_edits_wait_for_release.md) left as it was.

Peers' derived views of a held edit: plans/peers_preview_held_edits.md.

## 26. Server protocol

- **Authority names a head of its signer's choosing.** A record on authority from
  the organization's or an ancestor's log names an entry of it (`Authority`,
  `Link::head`), and `answer` checks the signer's right as of that entry alone:
  a manager revoked above at place N, with a server that takes it, can append a
  grant below naming a head before N, and devices take it (guides/blocks/keys.md,
  "Accepted limits", "Stale authority"). So can whoever stole a key retired at
  place N, an organization's or a block's, with the server: a record it signs
  naming a head before N checks on the honest whole log. No signed time orders the record below
  against the revocation above, so bounding it means choosing what a revocation
  above does to records signed on it: refused once anything after the named head
  lowered the signer (their honest grants then lapse with them, and are signed
  again by a manager who holds), or a commitment in the record below to the
  upper log's head as the server held it, checked against later entries. Done:
  a decision on which, recorded in decisions/, and `answer` refusing a revoked
  signer's record that names a head before the revocation, or a retired key's
  that names a head before the retirement, with a be-block-keys test.

## 27. Unread counts

- **An open block counted live, for a type that counts from its content.** A
  block's unread count is counted from its head and its signed log's latest
  entries (`crates/be-host-stack/src/unread.rs`). Chat, the only type that
  counts, counts from its log alone, and an append is counted as the server
  takes it (`Event::Appended`, `Counting::appended`), open or not: no seal
  delays it. A type that counted from its content would see an operation
  relayed in an open session only once the session seals it. Done, once such
  a type exists: while a replica of the block is open, the count comes from
  the replica's confirmed content as operations land (throttled, one module
  call per burst: a replica call that counts its confirmed content, as
  `replicas.unread` counts a head), falling back to the head when it closes,
  with a be-host-stack test that sees an operation relayed in an open session
  count before any seal.

## 33. Canvas

- **An empty text box left open at close is kept.** A text box the Text tool
  adds is written at once, in an undo group its typing joins, and dropped if
  editing ends with it empty; but the host commits an undo group its editor
  leaves open, so closing the canvas while the box is still empty keeps an
  invisible empty Text entity, as an undo step. Done: plans/deferred/canvas.md,
  step 4 (a draft written on its first keystroke), with its test.
