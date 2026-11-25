# Collected reviewed work (rebase-merge)

The description of [blockeditor-org/be3#1015](https://github.com/blockeditor-org/be3/pull/1015), kept here so it can be edited like any other file. Delete this file when the pull request merges.

One branch that collects reviewed work, so it lands together instead of as many small pull requests that conflict while CI waits. The commit list shows what is included. Each piece is one or a few commits on a linear history; merge with **rebase** to keep it.

Nothing is added until it has passed review and `./scripts/verify` has passed on the exact commit the branch moves to. The branch is treated like main: rebased onto main when main moves, and force-pushed only for that.

## Open work (resumed 2026-11-24)

Development resumed on 2026-11-24, with eight builders. Nothing below is on this branch yet. Branches are on origin.

**Being built:**
- Hosted blocks: 4e-2 rebased with its review fixes, then acceptance-2 rebased, then the backlog's "keep hosting through a closed editor, under verify load" flake.
- Restart carry (b): `journaled-edits-restart-carries` rebased, with its must-fix.
- `flush-after-revoke`, then the counter-seal flake.
- `editor-asks-outlive-reopen` (an editor's in-flight put, append or log read answered once across a reopen), fixing review points; then an editor's range read across a reopen.
- `flakes-server-runtime` rebased for review, then plans/one_way_in_the_block_stack.md.
- Host adapter 4g7a2, then 4g7b and on.
- The member list leak through the organization's log, then the owner transfer's accept step.
- plans/dialogs_are_dock_windows.md.

Landed on 2026-11-24:
- `chat-stall-3`: an editor's log read in flight when its replica reopens is answered; before, a fresh chat editor could wait forever.
- `block-rows-columns-2`: every column that names a block is listed with how forget_blocks treats it, and forget_blocks refuses a block a link still grants.
- `flakes-batch-2`: two flakes fixed at the source (be-server's WAL switch under load waits for the other connection, and a contained panic outside be-host prints one line instead of a 300 MB backtrace); `Streamed` folded into `Blob` with `read_span`; `be_block_type_host::Host` gets no-op defaults.
- `journaled-edits-restart-carries-4`: a restart carries what was numbered after its seal, on every replica, and journal replay carries a restart whatever seal the journal starts from.
- `editor-range-reads-outlive-reopen-3`: an editor's log read, part put, append and range read are each answered once across a replica reopen, an append never lands twice, and a release carries no editor calls.
- `owner-accepts-3`: an owner transfer is an offer the new owner accepts: offer, withdraw and accept are in the log; an acceptance needs the acceptor's current keys and View; a revoke, a ban or a revoked link ends the offer.
- `host-adapter-4g7a2-3`: be-root is worded and laid out as be-host, fills its window and uses be-host's menu; agent credentials are the host's bundles and the organization is optional (step 4g7a2).
- `one-way-in-apps-1b`: the frame registers a pan-and-zoom editor's zoom actions and one ZoomBar shows them; the logic grid, map and canvas zoom through it (verify: only the known flaky lease test failed, fixed separately).
- `seal-states-go-by-part-6`: a session's state seals by its own part, and an editor's put that a replica carries leaves out its own state parts.
- `windows-restored-chips`: on Windows, windows from last time wait in chips and restore from the start menu; two dead-code errors on Windows fixed.
- `one-way-in-apps-2`: one replace panel (ContentFileReplace) for the image, audio, PDF and game module editors; each file type says its extensions and MIME types, and pickers filter by them; m4a, oga, pnm and tga are recognised.
- `one-way-batch-3`: one way each in the block stack: one export of a subtree to a block file (refusing owner-only blocks and unchecked parents), one read-change-write loop (Peer::update), and the type-module WIT maps onto the block stack's own types with one wire form for hashes (verify: only the known flaky lease test failed).
- `one-way-in-apps-3`: step 5, a name too long is refused and the editor says why (`Editor::notify`); names taken from content are cut with `metadata::clip_name`; file types list their media types.
- `tools-batch`: one-way-in-the-tools steps 3-8 (browser rasterizer, build helper by content hash, Painting::render takes the original file, Storage::put_content_addressed) and be-tool-local stress. A manifest without an `executable` field, now required in `ManifestFile`, fails to parse, so its build is downloaded again. Untested: a real Chromium rasterization and Bunny storage HEAD.
- `invitations-no-pending-4`: an invitation is made whenever the organization shares outside (not gated by link sharing) and has no pending grant: its first opener becomes a contact with the access chosen, and a failed contact link leaves no share link. `links.grant_block` is gone.
- `host-adapter-4g7b1`: a root's embedded editors are served through its host (forwarded instances, relayed ops).
- `flush-after-revoke-6b`: a flush keeps its mark until a seal settles it, and seals what it answered ahead of once the block owes a seal it can make; fixes the lease flake. Backlog §32: a Host event held during a join can override its answer.
- `one-way-in-apps-4`: worker messages are typed, and a shared worker's jobs carry tickets so each answer goes to the job it answers (extras dropped).
- `kept-content-1-3b`: a kept subtree reads its showing Kept's context; overlays and context menus build their body on each open and drop it once gone; Scroll settles a fling or release on its nearest snap stop; an HTML overlay dropped while shown gives the focus back.
- `counter-seal-flake-2`: every installed host waker is woken; a replica that gains the lease takes a head it heard of while not the holder. (Verify: keep_hosting_outlives_the_editor_until_stop fails on mega too; a_block_just_made_in_a_folder_lists_the_folder_among_its_backrefs flakes ~9% under load on mega too.)
- `one-way-in-apps-5`: one block-tree walk for the files pane and the block picker (block_tree); a folder's rows get the files pane's row menu (block_menu; can_move_out_of takes a block-type lookup); plans/one_way_in_apps.md done.
- `host-adapter-4g7b2`: be-host runs its core under --root and serves the root's editors through Forwarded; be-root shows the session's editor full-window and the signed-in block stack; a root's paintings carry its apps' screens; an agent made for a root that stopped waiting is removed; root views compile only where --root does.
- `backrefs-flake`: the backrefs test waits for the server to make the folder and child before it edits the folder (test timing; 18/200 → 0/800).
- `host-adapter-4g7b2-tests`: tests for agent undo, a deferred agent and the stop after a protocol break; fixes a deferred agent taking the credentials of the one made before it.
- `dock-defaults`: docks derive ids (stepping past held keys), a `stage` short form, chrome panes, standard panels and side panel sizes; editors moved to the short form.
- `host-adapter-4g7b3a`: be-root shows the block children its session's editor places, opens the editor each placement names, settles them by key, and takes presses only where a child shows.
- `fonts-on-skrifa`: fonts on skrifa, harfrust and zeno in place of FreeType and HarfBuzz; shape plans, coverage and metrics cached; system scan reads only headers and cmap.
- `missing-object-not-unreadable`: a head whose objects are missing is not-yet, not unreadable; not-yet loads wait on the join, are bounded, and run no jobs on default content; host reads of a not-yet block fail at once.
- `hosted-blocks-acceptance`: hosted blocks step 4e (be BLOCK host), players carry on with the next host, a closing replica does not rejoin, kept hosts say whether they host now; export checks not-yet beside its content (wasi ICE).
- `kept-content-4e`: a sheet is one Scroll with sticky items, a bar range and snap stops; its grip lowers it at once keeping the content scrolled; a pull past its end fills with its surface; a scroll never anchors on a sticky item; pagers wheel and key back between stops.
- `hosted-blocks-followups-2`: the hosted blocks review suggestions; the host menu says whether a kept block is hosted here now; drive prints descriptions and bare text never matches them; export checks not-yet before asking the server.
- `motion-springs-4`: back slides and toasts move on springs that carry the gesture's speed; toasts swipe to dismiss and one let go after it expired while held still leaves; a toast its owner removes slides off; Movement::Enter/Leave removed.
- `no-held-keys` + `host-adapter-4g7b3b-4`: the gift system and held keys are gone; an editor asks for the keyboard to move into a child with a growing count in its layout commit, and the host acts at the commit (the child takes it if up, else the placer blurs; an ask that cannot apply ends); be-root moves the keyboard into asked children, tells placers child-focused, and takes active ones back (4g7b3b).
- `host-adapter-4g7b3c`: under a root, a child that shows takes its placer's frame (taken back from every child of the view by a press outside or Escape), and docks join across apps through one `Docks` and `host.dock`; be-host waits for blocks a root asks for before they are listed.
- `export-through-artifacts-7`: export goes through artifacts: an export is a block that follows its source, made again by the source's exporter, with its settings and fingerprint under separate keys, standing failures reported, and no parallel export APIs; `transfer::Open::content` returns a boxed future to avoid a rustc ICE on the web.
- `flakes-6`: a root counts as settled only once no app is starting or compiling and it has stepped with every answer it was given, and a held rejoin takes only its own next entry (same origin, base and sequence), replaying only entries past what it heard.
- `flakes-7`: only the present of a frame that was asked for meets a need, so a `frame-needed` heard before a present the app made unasked is still drawn (the stale "editor is opening" under be-root); the not-yet export test waits out the references read before holding answers.
- `flakes-8`: a `next` the sandbox answers inside a root's call counts as an answer the root has yet to step with, so the root does not settle with frames still to take.
- `host-adapter-4g7b3d`: under a root, previews (shown in their child's place, `ChildPreview::Composited`), blocks opened from root editors, the host's block catalog (`host.block-types`), and a placed pan-and-zoom child's view (`host.place-content`); an app asked for no screens is asked for no frames.
- `host-adapter-4g7b3e`: under a root, a child placed turned, faded, at its zoom, fitted and under occluders, and one app per staged app and one editor per block shared across places (other places draw copies).
- `host-adapter-4g7b3f`: under a root, editors' drags carried by the input router (`drag-block`, `drag-accepted`, Escape puts a carry down), and their menus, details, child-menu picks and leave-frame routed as be-host does.
- `host-adapter-4g7b3g` (dialogs): under a root, pickers, rename forms and Export dialogs as dialog children (`shown` = block or dialog, `host.tell-shell`); creations, artifact settings, host panels and web views are left in the plan.
- `host-adapter-4g7c`: the root shows the views its apps anchor (menus, pickers) outside their groups, stacked by open order; presses dismiss them as in be-host; keys go to a child an anchored view places and back to its owner on close.
- `flakes-9`: undo steps timed by when an edit was relayed; a stopping stack closes its connection at once and account reads retry on a new one; an offline edit merges with no base when the ancestor is held nowhere; a preview canvas gets no view until its stage is measured (no stray glyph, no pause).
- `keys-asserts-webviews`: input for an unannounced screen is dropped (no queue); a view that gets the keyboard is told the held modifiers as state with its focus; duplicate dock ids assert; web view leftovers (GTK/WebKit fixups, bootstrap packages) removed; an export stores the theme shown when its dialog opened.
- `host-adapter-4g7b3h`: under the root a dialog on a phone opens as a sheet, Escape keeps the keyboard in an active child, and a web view fails with its message as under be-host.
- `follower-fetches-heads`: a replica following its holder fetches the holder's seals (one at a time per block, skipping its own, cancelled at stop); a canvas with no measured camera takes no input.
- `sheets-motion-toasts`: springs overshoot once and land (SpringMotion); a sheet thrown or let go settles on its stop with the release speed; content flung past the top moves the sheet; the back gesture slides a dialog away; toasts shrink to fit a narrow screen.
- `flake-not-numbered`: an entry of the order that waits for its key counts as heard only once it opens, so it is not passed over when the key arrives (one cause of "1 submitted, not yet numbered").
- `join-left-suspicions`: of a block's joins on their way only the newest still out answers for the session, and a dropped join stops holding events once no other is out.
- `host-adapter-4g7b3i..k`: under the root each place gets a live editor of its own; a child taking the frame stays in place as its panel opens and closes; a creation child makes its block as under be-host, and a failed creation tries again once committed again.
- `checklist-on-canvas`: a new canvas card takes the size its child tells in the undo step that added it (one step per add, paste or drop, any resize mode); closing an editor commits its open undo groups; be-host reports no size until its app tells one or presents a frame.
- `host-adapter-4g7b3l..o`: under the root, an artifact's settings open in a dialog whose list escapes it; every host panel opens through host.show-panel and the root's statistics overlay is gone, with one statistics feed for all Performance panels; dialogs in an edge-anchored view open as sheets; a web browser tab says why it shows no page.
- `host-adapter-4g7d1..2`: under the root, be-root draws the one command palette (Ctrl+Shift+P or a frame menu's Command palette) with the focused editor's actions and those of each editor around it, gated as be-host gates it and shut while locked or in a dialog; F6 and Shift+F6 move the keyboard between the apps' views in be-host's order.
- `host-adapter-4g7d fix-ups`: F6 under the root stays within an app's modal dialog or menu and leaves a picker, as under be-host; the Performance panel painting no longer catches live statistics.
- `host-adapter-4g7e1`: under the root `--desktop` shows the desktop shell as the session's editor, its OS services left for 4g7g; the root tells its apps the host's wall clock and UTC offset.
- `host-adapter-4g7f`, `host-adapter-4g7e2a`: drive reads the root's accessibility tree under --root (own surfaces described only while asked, changes only); a root frame ending with its apps unstepped wakes its host; a desktop session docked under the root joins the desktop's dock. open/close/raise of app windows deferred to 4g7e3.
- `host-adapter-4g7e2a` follow-up: the docks learn every docked child before any is asked for its screens; the 4g7e3 plan split into e3a..d.
- `kept-content-5`, `kept-content-6`: one `Card` under popovers and dialogs (`PopoverPanel` and `DialogCard` gone); the colour wheel stays on the gpu (decision recorded).
- `headless-retire`: plans/headless_apps.md, fully built, deleted; its references point at guides/apps/runtime.md "Headless", and the real-deploy run is in plans/backlog.md §22.
- `kept-content-7`, `kept-content-retire`: one find_text and shared dock fixtures; plans/kept_content_and_one_scroller.md retired
- `headless-noise`: a headless runtime reads only the modules of apps with a host component
- `signed-logs-6`, `signed-logs-7`: content edits announce nothing, only log entries do; apps' sign/verify services removed; decision recorded, plans/signed_logs.md and plans/chat.md retired
- `hosted-blocks-5`: the share dialog was already built; step removed from the plan
- `host-adapter-4g7e3a`: the desktop host runs under be-root and its windows reach the desktop; be-root wakes its host when an app wants a frame after the root's frame was done
- `pure-guests-1`, `pure-guests-2`: pure guests' few WASI calls are answered natively and on the web, held to RFC 7539's vectors; the pure guests build in the guests' wasip1 plan (opt-level 3, pending the user)
- `pure-guests-3`: a module from bytes' spawner chooses its seed; the plan is built and deleted
- `one-way-host-1`, `-2`, `-5`, `-6`: one panic container the hook hears for every guest thread; one RootSettings in be-block; the push device state publishes itself; one FailureBudget for restarted guests and programs
- `host-adapter-4g7e3b`: the root shows program windows where its apps place them, from pictures the desktop host hands it; drive's settle waits for a shown program window to draw its configure
- `one-way-host-3`, `-7`: one shared test server fixture in be-host-server; the test harness is ordinary code in be_host_stack::testing (decisions/test_hooks_are_ordinary_code.md); FailureBudget keeps restart counts
- `dialogs-dock-1`..`3b`: a Window points at its anchor and may block its owner; a DockDialog drags within its window; dialogs are sheets on phones
- `hosted-blocks-6`, `-7`: the gem hunt, the first hosted block type, played through its host; a Join editor's frame offers no content actions and its row marks it joinable
- `android-link`: be-host links for Android again (Oboe on the NDK's shared libc++ beside V8's; decisions/one_cpp_runtime_beside_v8_on_android.md); the broad check links be-host's Android library and leaves out wasm test modules
- `demo-top-bar`: beui-demo's top bar fits a phone-wide window (actions as icons, theme under More); the styled ActionBar takes `labels` as a Prop.
- `members-private-2`: only an administrator is served the organization's whole log; an editor is served a proof path, so members no longer see every other member through it.

Landed on 2026-11-22 in one verify: signed logs 5b (`signed-logs-5b-10`), `backlog-keys-10b`, `block-rows-guard` and hosted blocks 4d2 (`hosted-blocks-4d2-5`).

**Decided by the maintainer, not yet built:**
- An owner transfer needs an accept step: the new owner accepts before ownership moves.
- Windows the Windows host restores at start get window chips in the desktop bar, like any other window.
- Members don't all see the whole member list.
  - Today `list-members` (`Store::list_members`) already gives the whole list only to administrators; any other member gets only the agents it made.
  - But there is a leak: a block's creator holds Manage on it, so every member who has made a block is served the organization's whole access log, and that log names every member (guides/blocks/keys.md, "Who is served what", left open there).
  - To build: narrow what a block's creator is served of the organization's log, for example a proof path of its own right, so that the log no longer reveals the members.
  - The personal organization is unaffected: it holds every account but has no log.

**Follow-up owed on landed work:**
- block-rows-guard: also guard the other columns that name a block (`access_log_authorities.above_id`, `block_edges.reference_id`, `blocks.parent_id`), or document that `forget_blocks` takes whole subtrees made by an account that holds no links.

**Hosted blocks 4e and step 4's acceptance** (plans/hosted_blocks.md): stacked on 4d2-5, which is now on this branch, so rebase both onto this branch's tip. Neither is verified.
- `claude/hosted-blocks-4e-2` @ a06baa44 (review: NOT READY). Still to do from the review:
  - refuse up front without Edit;
  - print "hosting" only once `seen.hosted` holds the block;
  - a second Ctrl-C exits at once;
  - hold the data-dir lock for the stack's whole life;
  - wake on `lease_revision` instead of a 250 ms tick;
  - the standby comment;
  - key the cache by account;
  - the S1 race, in the backlog or fixed.
- `claude/hosted-blocks-acceptance-2` @ 349d920f, on 4e-2 (not reviewed):
  - The closing test `players_carry_on_with_the_next_host_once_the_first_closes_the_world`, with two raw-peer players (the new `Server::player`) who hear and are answered by the next host.
  - The host menu's kept row says "Hosting here now" or "Kept here, standing by"; the painting `the_menu_lists_what_this_device_keeps_hosting_with_stop` changed.
  - Two 4d2 races fixed:
    - `Replicas::rejoin_block` skips a closing replica, so a reopen no longer waits forever;
    - `hosting::heard` takes Host only while the replica is open.
    - Stress went from 5 failures in 8 to 0 in 12.
  - The `a_joined_channel` flake fixed in its helper: the new `take_member_events_until` and `take_channel_events_until` wait on the page's own record. It went from 10 failures in 32 to 0 in 32.
  - Not yet written: unit tests for the stale Host after leaving, and for rejoining while closing.
- Not reproduced: an edit stuck at "1 submitted, not yet numbered" after a sibling device switched organization, which the server never sequenced. A host-test attempt passed 22 of 22 runs and was discarded. Details are in plans/backlog.md §15 on acceptance-2.

**Built, not reviewed or verified:**
- `claude/flakes-server-runtime` @ 517b9857 (on 0628fbdc): two flakes fixed at the source.
  - be-server under load: two connections switching a new SQLite database to WAL failed with `SQLITE_BUSY` without calling the busy handler.
    - The new `be_block_sqlite_schema::use_wal` waits out the other switch and retries; be-server and be-block-client use it.
    - A deterministic test covers it, and guides/blocks/server.md now says to switch with `use_wal`, never the bare pragma.
    - It went from 5 failures in 800 runs to 0 in 1,200.
  - be-host-runtime in parallel: the two tests that panic on purpose symbolized a 360 MB harness's backtrace, about 300 MB per run, until the memory cgroup killed them.
    - `panic_guard::contain` now prints a contained panic as one line where be-host's hook isn't installed.
    - It went from 201 failures in 400 runs to 0 in 1,000.
  - guides/testing_a_gui.md gains how to repeat a crate's own test under load.
  - Handoff notes:
    - `Running::with_mail` still returns before the server opens its store;
    - other places drop a `serve_with_mail` result;
    - a plain whole-crate be-host-runtime run has pre-existing failures, which is why it is `run_apart`.
- `claude/journaled-edits-restart-carries` @ b1722b23 (on b5d4bcfa; review: **NOT READY**, one must-fix): restart carry (b), which removes the interim hold-and-reseal now on this branch.
  - At a due seal the holder restarts at once, based at the seal. Every replica rebuilds the content at the seal and carries each entry numbered after it onto the fresh commit with `LiveEdit::carry_over`. Journal replay does the same.
  - Removed: the wait for a full seal, `restart_short`, hold-and-reseal and `report_overdue_restart`.
  - Must-fix: when a journal's base isn't the restart's seal T, `replay_entry` counts the restart but applies the later entries to pre-restart content, silently mis-applying them. This happens when a lost seal's rebase leaves the journal on an earlier seal.
    - Fix it one of two ways: build the content at the seal from the entry's own state and commit, then carry, in `loaded`'s async path; or refuse and open on the head with an error.
    - Add a test for that journal shape.
  - Review suggestions:
    - a clearer `CARRIED_CLIENTS` comment;
    - a `debug_assert` that the log covers (T, S);
    - "logged" not "applied" in the decision's step 3;
    - a backlog link to plans/seal_states_go_by_part.md.
  - New tests:
    - a holder streaming faster than a seal;
    - a holder whose seal a peer outpaces, where a third replica opening on the journal must match both;
    - a restart on a seal the order moved past.
  - Docs:
    - decisions/a_restart_carries_what_was_numbered_after_its_seal.md is new.
    - The Restarts section of guides/type_modules.md is updated.
    - The backlog §7 entry is removed.
    - What remains of the plan is plans/seal_states_go_by_part.md: the 4 MiB ceiling when not alone.

**Work in progress:**
- `claude/flush-after-revoke-5`: fixes `a_publish_right_after_a_revoke_lands_without_waiting`, which stalled with "owes a seal: true, holds the lease: true, publishes unanswered: 0".
  - A flush answered while a block could still come to owe a seal, and nothing asked for that seal before the quiet spell, which a held clock never brings. Two ways, both confirmed with prints:
    - The edit waited in the lane behind the revoke, and the flush, which needs no block, ran and answered ahead of it (the test's own failure, which by now fails alone on the tip).
    - The block's replica did not hold the lease when the flush answered, and the lease came to it after.
  - Fix, in be-block-type-host: `Replicas::flushed` marks the blocks a flush answers on while they are unclean, unreported or not the lease holder, or every open block while commands sent before the flush still wait (`flush_waits`, until the worker's `lane_settled`). A marked block is asked to seal once (`seal_flushed`) when it reports a seal it owes and can make, or when a seal asked of it ends. Status reports and seal answers drive it, so it does not spin. A reload holding seals back clears `owes_seal` until it ends.
  - Tests in be-host-stack, each failing on the tip with the stall's signature:
    - `a_flush_that_runs_ahead_of_an_edit_waiting_behind_a_revoke_seals_it_once_it_runs`
    - `a_flush_answered_before_the_lease_comes_back_seals_once_it_does`
  - The stall report shows whether the replica holds the lease.
  - Dropped: the worker's `flushed_ahead` reseal, the earlier attempt test, and a guest-side re-ask (`seal_asked`), which raced the flush's own late keep.
  - `-6` on top, folding in the review:
    - A block's flush mark stays while its seal is asked, so an ask never sent or passed over is asked again. It also stays on a block that doesn't hold the lease, until a later flush. A replica opened while a flush waits on the lane starts marked.
    - `a_device_hosts_only_once_its_component_claims_the_lease` was flaky because the other device could leave before this device's rejoin. The test now waits for the rejoin's answer (`Seen::session_joins`).
    - Not fixed: a Host event held during a join can replay after the join's answer and override it. A trace showed a late rejoin answered "no one hosts", then a held event made this device host again.
  - Separate and not started: the counter-seal flake `an_edit_sealed_offline_onto_a_seal_the_server_lost_lands_once` ("never counted 5, last 2"). After reopening, the replica opened on the older seal and was never told the server's newer head, which looks like a lost head announcement.

**Not started:**
- Host adapter (plans/host_as_platform_adapter.md): 4g7a1 is on this branch, and no branch holds unlanded work. `claude/host-adapter-4g7a` (56ba435c) is superseded and can be deleted.
  - Next is 4g7a2, be-host's wording and layout:
    - be-root sizes to the window rather than the fixture's 400×240 `SIZE`, the fixed 152 pt `DIALOG_SIZE` and `MENU_BUTTON`;
    - be-host's titles and copy;
    - the menu's entries as be-host shows them, by the organization's role or company, with unsent edits only when there are some;
    - the core-view fields that needs: organization, role, escrow warning and sync.
    - Then re-accept the `a_root_opens_its_menu` painting.
  - Then come 4g7b–f, in order:
    - app views;
    - anchored views;
    - editors and the camera, with the focus chords;
    - desktop windows;
    - splitting drive's commands.
  - Last, 4g7g: be-host brings the host's pieces back under `--root`. The root becomes the default only once 4g7a2's layout and the host tests over be-host's src/ui pass with their paintings compared.
  - Open questions:
    - Bare `--root` runs be3.root, but only `:native` stages it. Whether `:dev` or a shipped bundle should stage it before 4g7g is undecided.
    - Some guest glue is duplicated between be-root and the fixture: the Notice→Toast mapping, answer-notice, and input/Embedder. Move it into be-sdk-root or be-sdk-beui-root if a third root appears.
- The deploy dry-run (`./scripts/deploy --dry-run`) and the Chromium web test.

**Dropped:** `claude/quieter-unlisted` (01e8bfb2), whose log quieting backrefs-4 superseded.

## Tracker rows to file

`./scripts/be` can't run here: the pinned bootstrap predates "download be unpinned", and only CI re-pins it. File these once it works:
- A one-off "1 submitted, not yet numbered" host-stack flake, not reproduced.
- A replica's online flag lags behind its connection's reconnect.
- A diverged replica's journaled edits after it closes (plans/backlog.md, 7).
- An administrator's new phrase can race a member's write (plans/backlog.md, 18).
- A mailbox target for letters a client sends on its own (server API 4b may add it).
- An `Editor::until_taken` test helper.

## Manual steps for the maintainer

Only what needs a person to do something. Some apply only once the piece named is on this branch; check the commit list.

**Deploy workflow, in order:**
- [ ] Generate an ed25519 SSH key used only for deploys. Uncomment the "CI deploy key" entry in `servers/common.nix` with its public half, land that, and deploy servers/api by hand (guides/servers.md).
- [ ] Create the GitHub environment `deploy`, with required reviewers, and add the secrets `BE3_DEPLOY_BUNNY_STORAGE_KEY`, `BE3_DEPLOY_BUNNY_API_KEY`, `BE3_ANDROID_RELEASE_KEYSTORE` (base64; without it, untick the APK), `BE3_DEPLOY_SSH_KEY`, `BE3_DEPLOY_KNOWN_HOSTS` (`ssh-keyscan api.be3.pfg.pw`, checked against the real host key) and `BE3_NL_TOKEN`.
- [ ] In the Bunny pull zone, send `Service-Worker-Allowed: /` on `/data/*`, and give `/`, `/index.html` and `/live/*` a one-hour TTL and `Cache-Control: no-cache` to browsers. Delete the stale `/web/downloads.html`.
- [ ] After the first deploy, load https://be3.pfg.pw and check that the service worker's scope is `/`, with a script under `/data/BUILD/web/sw.js`.

**Data:**
- [ ] Before deploying this branch, stop be-server and delete its data, and delete every device's data directory: the protocol, schema, seal format and local accounts all changed with no migration.
- [ ] After deploying, make the first account with `be-server login-link --auth-origin https://api.be3.pfg.pw --email YOU` on the server (guides/hosting.md). Passwords are gone, and sign-in codes are delivered only to the server's journal.

**Provider sign-in (OIDC), if wanted:**
- [ ] Register a web application for each provider, with redirect URI `https://api.be3.pfg.pw/auth/oidc/ID/callback`.
- [ ] Write the providers file next to the data directory, readable only by be-server (format in guides/hosting.md), add `--oidc-providers FILE` to the be-server service in servers/, and deploy.

**Checks only a device can do:**
- [ ] Android: install the debug APK and open an app to confirm guests run on V8. Install a UnifiedPush distributor (ntfy also needs `--push-host` on the be-server service) and run the push checks in guides/running/android.md.
- [ ] Windows: run `BEUI_LAYERS=space ./scripts/buck run //crates/be-host:native -- --desktop` and check that no window is captured until picked from a menu, that only a click in content focuses and locks the mouse, and that Ctrl+Alt+Space defocuses.
- [ ] Firefox and Safari: in a text area, check that a line just wide enough keeps its last word, and that a selected newline highlights at full height.
- [ ] After the first deploy, check that `be` downloads its apps and type modules from the release.

**Build server:**
- [ ] Cap how many host tests a NativeLink worker runs at once by memory, not only CPU. Under a full verify, a crowded worker starves an app's step for 10 s.

**Repository:**
- [ ] Re-enable the CI workflow. Only CI publishes and pins a new bootstrap (guides/bootstrap.md).
- [ ] Once CI publishes bootstraps again, build the bootstrap steps of plans/one_way_in_the_tools.md (one acceptance of paintings shared with be-tool-verify, and the bootstrap's shared helpers). They wait on a new bootstrap release and pin.
- [ ] Delete stale branches: `claude/headless-c-7`, `claude/headless-c-7-rebased`, `feat/menu-intent-typeahead`, `feat/menu-intent-typeahead-2` to `-4`, `fix/stack-test-flakes`, `claude/host-adapter-4g7a`, and other numbered branches whose last number landed.
- [ ] Optional: AGENTS.md's principle "No in-app reach to strangers" could mention that a share link also counts as an out-of-band start (decisions/share_links_are_an_out_of_band_start.md).
- [ ] Delete this file, then merge this pull request with **rebase**.
