# Backlog

Depends on: none.

The running to-do list, grouped. Every item says what "done" looks like.
Fixed items are removed as they land.

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

- **A stacked dock's old bar over the child that replaced it.** On a phone, with presents
  late (`drive present late`), the frame after a document is opened from the files sometimes
  shows the files' bar over the document's undrawn view, and sometimes the document's bar:
  the children of the root's layout can be applied before the present that paints its new bar,
  as happens whenever a present misses its frame, and which lands first varies between runs
  (about 1 run in 6 under `stress`). Done: that frame shows the files under their own bar, or
  the document's bar over its view, every run, and
  `on_a_phone_opening_a_document_from_the_files_steps_frame_by_frame_with_presents_late` records
  each frame of the opening (`HostTest::record_frame`) in a painting.

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

- **A replay's reads fetch what they are refused.** `Scope::check` (be-block-type-host's
  modules.rs) asks `Peer::block_of_part` of any part a module's `replay` names, which may
  fetch its object before the read is refused as another block's; the server still decides
  access. Done: a part's block found without fetching what this device does not hold.

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
- **An answer withdraws its hits while it runs again.** When an index's head
  moves, a standing search runs again (`forget_answers`), and until snippets are
  cut again the answer shows no hits, so a page reading in that gap briefly shows
  no results. a_local_account_finds_the_text_it_wrote hit it (fixed in the test by
  reading hit and snippet in one wait). Done: the previous answer stays until the
  new one has its snippets, shown by a test that reads across a head change.

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
  verify keeps it) is what to read.

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
  it reopens, it still stands by (its replica leaves the session as the tab closes).

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
  The components' own checks today: toast.rs (`DRAG_THRESHOLD` on x),
  number_input.rs (its own 2.0), tree.rs (`ROW_DRAG_THRESHOLD`, 6.0),
  beui-dock styled.rs (`DRAG_THRESHOLD`), and the core's touch checks in
  beui-core interact/pointer.rs. A slider must still land where it was pressed
  and move by the whole distance once the threshold is passed, so the core
  hands the first `on_drag` the movement from the press, not from the
  threshold; expect paintings of slider and splitter drags to move.
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

## 20. Keyboard

- **A view given the keyboard before it is ready.** When the host gives a
  view the keyboard before its content is up (a joined text's dock tab view
  given it the moment its group is shown again, before the app's subregion
  hears of the child placed in its stead), the view takes the host's focus but
  its app does not focus the field it would otherwise: the document's focus is
  already on another node inside that view's window, so neither the window's
  kept focus nor `focus_into` (which waits one layout) puts it on the text
  input. Found while fixing Ctrl+Tab between texts. Done: a
  view given the keyboard early focuses its field once it is up, shown by a
  host test that gives a text's view the keyboard before its content arrives.
  Left: a host test has no way to hold an app's content back while the host gives its view the
  keyboard, so the race cannot be shown yet; that needs a test hook first (delaying a
  subregion's child placement, say). Where to start: beui-core `window/hosted.rs`
  (`window_focused` gives back the window's kept focus, else `focus_into` its root only when the
  focus is not already inside it) and be-app-text's `caret_when_given_the_keyboard`, which
  focuses the field only for an empty text.

## 21. Component macro

- **Internal trait names in a prop's error notes.** A value a children slot or
  an exact prop does not take is reported with the right headline and label,
  but rustc's "required for ..." notes under it name each blanket impl the
  obligation passed through: `TakesOptionalBlock`, `TakesBlock`, `ChildShape`
  and `TakesChildren` (`a_frame_takes_one_child`), `Exact`, `TakesWith`,
  `ChildShape` and `TakesChildren` (`a_loading_hands_its_content_no_value`
  in compile_fail/beui). rustc prints one such note per impl and has no stable
  way to hide them while keeping the leaf trait's `on_unimplemented` headline:
  `#[diagnostic::do_not_recommend]` on an impl drops the impl from the chain,
  so the error is reported against the outer trait and its message instead.
  Done: those notes name only the declared prop type and the slot's own trait,
  or are suppressed, shown by the compile_fail pins' full text (the checker,
  be-tool-buck's `compile-fail`, matches only part of the headline now). Left
  until rustc can do it, or the slot's bound is restructured so the leaf trait
  is asked directly by the builder.

## 22. Headless runtime

- **be-host-runtime without its gpu and presenter.** `be` links
  be-host-runtime to run host components headless (guides/apps/runtime.md, "Headless"),
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
- **A deployed `be` running apps.** Reading manifests and type modules from a
  deployed `be`'s release (checked against the `app-hashes.json` its
  `build-info.json` pins) and `be BLOCK host` have run only against apps staged
  beside a `be` built here, never a real deploy: only releases deployed after
  them name their `apps`. Done when a deployed `be` has read a block type and
  hosted a block from its release, or a test serves a release as the mirror does.

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
- **CI still installs Python Playwright for old pull requests.** ci.yml's
  "Install the browser preview runtime" step serves only checkouts from before
  the browser rasterizer ran `render.mjs` (those with
  crates/be-tool-browser-rasterizer/requirements.txt); a newer checkout skips
  it. Done: no open pull request lacks `render.mjs`, and the step is deleted.
- **The browser rasterizer is about 650 MB.** `browser_program` copies
  chrome-headless-shell (about 260 MB), node (about 120 MB) and the
  `buck/sysroot:amd64-browser` libraries (about 250 MB, mostly Mesa's `libLLVM`
  and `libgallium`, which `libgbm1` brings in), and CI downloads it all to draw
  the previews. Check whether chrome-headless-shell drawing with
  `--disable-gpu` (SwiftShader) needs `libgbm` and Mesa at all, and trim the
  sysroot to what it loads; if it cannot shrink, render the previews remotely
  as buck actions instead. Done: CI's download for the previews is
  measured and well under its present size, and the previews still draw every
  `.browser.paint` the same.

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

- **Imported history in a re-export.** An export carries only a block's own
  log entries (`Peer::export_log`), and `transfer::decode` refuses an entry
  written in another block, so exporting a chat with imported history drops
  that history from the file. Done: a file carries a block's history as its
  own run, each entry naming the block it was written in and verifying
  there, and an import of it shows that history again, with a
  be-block-client test exporting and importing a chat twice.

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

- **A failed drain waits for its timer.** A revoke then a publish on an open block
  once hung `flush_and_send` until its 60 s limit; it no longer reproduces
  (be-host-runtime `a_publish_after_a_revoke_lands_on_an_open_block`). One
  mechanism would show it: a drain that ends `Drained::Failed` drains again only at
  `OUTBOX_RETRY` (30 s, be-host-stack `retry_at`), which a driven clock never
  reaches, and which delays a publish by 30 s in the product. Done: what makes a
  drain fail after a revoke is known, and a failed drain drains again on a signal
  (keys arriving, a rotation finishing) rather than only on the timer, with a test.

## 29. Loading content

- **A large blob's header loads the whole node tree.** `Peer::load_first_part`
  fetches every node of the manifest's tree (`Vault::outline`, `missing_nodes`
  walk every level), O(leaves/fanout) objects, though `Blob::from_stored` needs
  only the path to the first part's end: 22 node objects of 1,737 in
  be-block-client's `a_large_image_is_read_by_span_without_downloading_all_of_it`.
  Done: loading the first part fetches only the nodes on the path to
  `first_end()`, so opening a large blob is O(depth), with that test's bound
  on the objects held after reading the header back to a constant.

## 32. Host events

- **A Host event held during a join can override the join's answer.**
  be-block-client's connection.rs replays the Host events it held during a
  join after the join's answer. If the server sent one before it handled the
  join, that stale event wins over the fresher answer: in a failing run of
  `a_device_hosts_only_once_its_component_claims_the_lease`, the rejoin's answer
  said no one hosts, and a held `Host { Some(us) }` then named this device host
  again. Done: a held Host event older than the join's answer is dropped, and a
  test sends one in that window and sees the answer stand.

## 33. Canvas

- **An empty text box left open at close is kept.** A text box the Text tool
  adds is written at once, in an undo group its typing joins, and dropped if
  editing ends with it empty; but the host commits an undo group its editor
  leaves open, so closing the canvas while the box is still empty keeps an
  invisible empty Text entity, as an undo step. Done: plans/deferred/canvas.md,
  step 4 (a draft written on its first keystroke), with its test.
