# Work status (paused 2026-11-10 ~02:20 UTC)

All agents are stopped. Every in-progress branch is pushed to origin. Work that wasn't committed when the agents
stopped is committed as `wip:` and is unverified. Nothing below has merged unless it says so. This file isn't meant
to be merged: delete the branch `docs/work-status` once the work is picked up.

Everything needed to continue is in this file and on origin.

## How the work was run

Continue the same way, or however suits.

- **One builder agent per plan or fix.** Each worked in its own git worktree off `origin/main` and committed in the
  form `type: message`. It never force-pushed: a branch that needed pushing again after a rebase went under a new
  name, which is why some branches end in `-2`, `-3` or `-4`. It didn't open a PR. It wrote the PR title and body to a
  file and reported back.
- **A separate reviewer agent per branch.** It was read-only, using `git diff origin/main...BRANCH`, and ranked its
  findings with file:line, a failure scenario, a fix, and blocking or not. The builder fixed them, and a reviewer
  checked the fixes again ("delta review"). This repeated until nothing blocked.
- **Reviewed branches were batched into one PR at a time**, with `gh pr merge --auto --squash`, since the merge queue
  and CI take about 15 minutes per run.
- **Every branch ran a full `./scripts/verify` before it was pushed as ready.**
- **Building on this VM, which has 15 GB of memory:**
  - Builds ran on the NativeLink build server, with `BE3_BUILD_SERVER=nativelink BE3_BOOTSTRAP=cargo`. `cargo` is
    needed until a bootstrap release that knows the server is pinned.
  - Each worktree has its own buck2 daemon, about 1.3 GB, so at most two builds ran at once, and each stopped its
    daemon (`./scripts/buck kill`) when it finished.
  - When the server was busy or down, builds fell back to local buck2 (guides/running/local_buck2.md). That ran in
    one checkout with a `.buckconfig.local` that names the local execution platforms, one build at a time.
- **Reaching the build server** (guides/build_server.md):
  - `BE3_NL_URL` is `https://box-node-f265b525733a03c8-8099.on.boat.dev/?_token=TOKEN`. TOKEN is `NL_TUNNEL_TOKEN`
    in the boat environment `nl-scheduler`'s secret file `nl-tunnel.env`, which
    `boat --json env info nl-scheduler` shows.
  - `BE3_NL_KEY` is a boat key that can resume the scheduler.
  - Each can be an environment variable or a file in `~/.config/be3` (`nl-url`, `nl-key`).
- **Builder VMs.** Four small boat VMs (`be3-builder-1` to `-4`) each ran an agent with its own checkout. They are
  interrupted, not deleted, and have an 8-hour lifetime from about 00:35 UTC. Their work is pushed (below). Delete
  them with `boat delete ID` (`boat list --all` shows them).

## Open PR

- **#829** (`feat/backlog-3`): backlog item 1 (push, don't poll: editors read the host only after `changed()`, via
  `ChangeFlags`) and the files pane's paste (copy then paste links, and a cut link moves).
  - Reviewed twice. It is otherwise ready, but CI fails on
    `be-host-tests:typing_right_after_adding_notes_reaches_the_notes`: a real regression that fails in every full
    verify with this branch.
  - The fix in progress is `fix/notes-typing` (below). Add its commit to the PR (or merge it into `feat/backlog-3`)
    and let CI rerun.

## Branches ready to merge (reviewed, nothing blocking, verify passed)

Batch these into the next PR(s) after #829. Rebase onto origin/main and run one verify first.

- **`feat/html-core-2`:** most of plans/html_core.md:
  - TextInput selection on HTML;
  - forms posting beui's own fields (disabled fields don't post; `ChoiceOption` has a `value`);
  - themed scrollbars;
  - TreeNode text hooks;
  - paste and back on HTML (`beui-core-html/src/back.rs` keeps one history entry while something would take Back,
    and finds the target as native `Dismiss::Back` does);
  - removing beui-font-browser;
  - no inspector on HTML;
  - fixes for two `//crates/be-host:web` build errors that verify doesn't build.

  Its non-blocking follow-ups are on `feat/html-core-3` (below).
- **`fix/nl-wake-on-redirect`:** build server fixes.
  - The bootstrap counted any HTTP status below 500 as "server up", but a stopped boat sandbox's port answers 302. So
    it never woke the stopped scheduler, and every build failed with "re-relay: the proxy answered 302 Found". It now
    wakes on any 3xx or 5xx.
  - `new-scheduler.sh` gave the scheduler an 8-hour boat lifetime, which stopped it mid-build, since nothing extends
    it. It now uses `--no-auto-stop`, and the controller's own idle stop is what stops it. The running scheduler has
    already been changed by hand.
- **`fix/host-palette-follow-ups-2`:** the command palette's last review findings:
  - a frame-menu pick lets an app open the palette only in the next two layout commits;
  - hidden views' actions are pruned;
  - anchored views' actions are offered;
  - window hosts are followed through layer changes instead of being compared every frame.

  Its base is an older main, so rebase it.

## Branches in progress

For each: what it is, its state, and what is left, including review findings not yet fixed.

### `fix/notes-typing`, which #829 needs

- **What it is:** one commit on `feat/backlog-3`, "keys typed right after Add notes survive a late notes editor".
- **State:** its last two test runs failed, and the full verify was cut off.
- **What is left:** find why `typing_right_after_adding_notes_reaches_the_notes` still fails.
  - **Clue:** the test loses typed text, not just delays it, whenever the host-stack worker loop stalls. #829's change
    and the search indexer both made it fail.
  - **Suspect:** the keyboard gift (#825: an editor hands the keyboard to a child block, and keys typed before the
    child is ready are held). It probably drops held keys, or never delivers them, when the child is late. Owed keys
    are delivered only when the view's revision changes.
  - Fix it at the source and run the test many times under load. Don't sleep and don't add a timeout.

### `feat/chat-1c-2`: plans/chat.md 1c, unread counts

- **What it is:**
  - Read places in owner-only settings (`ChatReads`), keyed `chat/device/message`: each device moves only its own
    place forward, the furthest counts, and a device drops only places it has seen.
  - A chat counts as read only while its view is shown, has the keyboard (per view), and is scrolled to the end.
  - App documents now start without the keyboard. A restarted app's focused screen gets `Focus` again, and a closing
    focused view sends `Focus(false)`.
- **State:** the review's two blocking findings are fixed: the keyboard not coming back after an app restart, and
  dropping read places losing a concurrent forward move. So are its non-blocking ones. The last commit is verify's
  fixes.
- **What is left:** run one verify to confirm, then a delta review of those fixes. One side effect to check: a view
  given the keyboard before it is first listed keeps it until it is listed.

### `feat/chat-1b-3`: plans/chat.md 1b, "Sending"/"Not sent"

- **What it is:**
  - The editor asks the replica to follow what the server lacks (`FromEditor::FollowLacking`), and the replica
    answers with the edits made on this device that the server doesn't hold yet (`ToEditor::Lacking`).
  - Chat shows those messages as "Sending", or "Not sent" with Send again and Discard once refused.
  - Type protocol changes:
    - `append` answers whether the server took the entries;
    - `publish-outcome` gains `kept`;
    - a new `drained(refused)`;
    - `opening` gains `journal-kept` and `commits-kept`.
- **State:** built on `feat/chat-1c-2`. The fixes for the second review are in, and verify passed. `feat/chat-1b-2` on
  origin is an older push. The review found these, and the fixes for them haven't been reviewed:
  - a drain while the chat was closed left it "Sending" forever;
  - two quick sends left the first "Sending";
  - a follower's stashed or handed-over edits showed as sent too early;
  - a drain race;
  - the cost of tracking with no follower, and pruning;
  - a handover with an append in flight;
  - the guide claimed only a host restart loses the list.
- **What is left:**
  - a delta review of those fixes, then merge after 1c;
  - known and documented: after a host or type-module restart, edits still in the outbox show as sent;
  - `drive` can't cut the server, so there is no host test for 1b.

### `feat/notifications-4`: plans/notifications.md steps 1, 2 and 7, and the server half of 6

- **What it is:**
  - Web Push: RFC 8291 encryption, in a new crate be-lib-web-push, with RFC 8292 VAPID.
  - FCM (HTTP v1) behind `--fcm-credentials`.
  - Endpoints and modes (all, mentions only, off) are stored on the server; the server pushes only to viewers with no
    open connection.
  - The sender's account is vouched for by the server, and the sealed part includes the block and send time.
  - Caps: at most 32 sends at once, and queues of 16,384 server-wide and 256 per account.
  - A `notify` host call, a modes watch pushed on changes, and a mute submenu in the block menu.
- **State:** `-3` passed its second review. `-4` adds a WIP commit for the push-host allowlist, unverified.
- **What is left:**
  - **Finish the allowlist.** The user decided the server must not be an open POST relay:
    - accept Web Push endpoints only on known push services by host: FCM, Mozilla autopush, Apple Web Push and
      Microsoft WNS;
    - add `--push-host HOST` (repeatable) for operator-added hosts, such as a self-hosted UnifiedPush or ntfy;
    - refuse others at registration and at send time;
    - write `decisions/push_endpoints_are_allowlisted.md`. Rejected: any public host; challenge verification, which
      push services don't support; and a push gateway, which only moves the problem.
  - **The second review's non-blocking findings:**
    - N1: cap concurrent sends per push host, and drop from the longest queue;
    - N2: key failure counts by account, organization and address, and check the key is a valid P-256 point when it
      registers;
    - N3: seal the sender as well;
    - N4: receivers drop duplicates (for the plan's receiving step);
    - N5: only one sender task per account;
    - a test that the semaphore caps sends.
  - **Left out, as the plan says:**
    - the web subscription and service worker;
    - Android (UnifiedPush and FCM clients);
    - a Firebase project: a service-account key for `--fcm-credentials`, and `google-services.json`.

### `feat/search`: plans/search.md steps 1, 2 and 4

- **What it is:**
  - Each top-level workspace gets one index block with exactly its parent's access.
  - The host indexes a block on seal, using the `search-text` request from #827, then queries.
  - A `search-watch` app resource.
  - The files pane lists hits, with the match highlighted.
  - Quick open lists hits after name matches.
- **State:** WIP commit. Its last full verify failed on the new `quick_open_finds_a_word_typed_in_a_text_and_opens_it`
  and on the notes-typing test. The newest commits move the indexer's work out of the worker loop, unverified.
- **What is left:**
  - Verify, and fix those two tests. If notes typing still fails, ask for search text from a task.
  - **Not reviewed yet:** needs a full review.
  - Step 3 (`changed-since`, catching up, masking blocks that leave the tree) needs a durable per-block change counter
    on the server.
  - Opening a hit at its location needs a field on `OpenableBlock` and `OpenBlock`.

### `feat/html-core-3`: follow-ups on `feat/html-core-2`

- **State:** WIP commit, unverified.
- **What is left:** finish and verify, including `./scripts/buck build //crates/be-host:web`, then a delta review.
  - The back sync recomputes its target on every requested frame. Use a dirty flag set by `changed()` and by layers
    opening and closing.
  - A reload while the current entry is the page's own back entry strands it. On start, `replaceState` it or treat it
    as already pushed.
  - Taking an entry back is asynchronous, so a push in between can race it. Delay pushes while a retraction is
    pending.

### `feat/text-oversized-edits`: plans/text.md step 3, and deletes the plan

- **What it is:**
  - An oversized operation's bytes go by `put-part`, and the operation is sequenced and relayed by its ref.
  - Replicas read the part before applying it, holding back what follows.
  - The old `Moved`/`Absorb`/replacement path is removed.
- **State:** targeted tests passed. A full verify hasn't run, and the last small changes are uncompiled.
- **What is left:**
  - Verify.
  - **Not reviewed yet:** a full review, since it changes the session protocol.
  - Known gaps:
    - a follower whose put fails retries only on its next peer message;
    - a stateless `replay` can't read payload parts;
    - an owner demoted while its put is pending isn't tested.

### `feat/authorship`: plans/authorship.md

- **What it is:**
  - An Ed25519 signing key derived from the identity secret.
  - Contact cards carry it, and the server stores it; there is a `get-signing-keys` call.
  - `sign` and `verify` host calls.
  - Chat messages are signed, and show whether their author signed them.
- **State:** the core crates (be-block-keys, be-block-client, be-server, be-host-stack) built and their tests passed.
  The WIT, be-sdk, be-sdk-test, be-host-runtime and chat parts were never compiled.
- **What is left:**
  - Compile, run `a_message_shows_whether_its_author_signed_it`, and verify.
  - **Not reviewed yet:** a full security review.
  - Rebase after the chat branches, since it touches be-type-chat.
  - Left in the plan: deletions and reactions aren't signed, an older signed edit can be replayed, and arrival times.

### `fix/review-follow-ups`: based on `feat/backlog-3` (#829)

- **What it is:**
  - The host alone owns a cut link: the `paste-blocks` answer gains `cut-from`, and the files pane's own record is
    gone. A failed or used-up cut falls back to linking.
  - Chord labels say Cmd and Option on Apple systems (`KeyChord::label`, `name_keys_as_apple`).
  - New tests for the palette's Split right, Split down and Close tab actions.
- **State:** items 1 and 2 are tested. Item 3 and the wasm-only parts of item 2 are uncompiled.
- **What is left:** `./scripts/test the_palettes_`, then verify, then a PR text. Land it after #829.

### `fix/flaky-host-tests`

- **What it is:** root-cause fixes for host tests that fail under load:
  - `a_new_image_asks_for_its_file_in_the_image_editors_own_dialog`;
  - `the_storage_page_says_what_the_server_counts`;
  - `the_members_dialog_lists_an_organizations_members_and_their_roles`, which timed out at 30 s in CI;
  - the painting `the_add_menu_opens_at_the_files_button_that_asked`, which flips between 258 and 259 shapes.
- **State:** a WIP commit of 26 files, unverified, with no report.
- **What is left:** read the diff, finish, and show each test passing many runs in a row under load.

## Plans not started, or waiting on a decision

- **plans/api_simplification.md:** not started. Its steps were waiting for the protocol-heavy branches above to land.
- **plans/chat.md 1d and 2:** 1d needs a type-side put-part hook; 2 needs notifications.
- **plans/backlog.md:**
  - item 2 (bar drops across layers) needs a decision on the dock state model;
  - item 3 needs a host-test harness for the web build;
  - item 5 (lazy lists) needs a decision on what reading an unloaded part returns;
  - item 4's rest: Android and macOS need devices, and refs in clip items and rich text marks are large;
  - item 6 (one keyboard-gift record per editor) is the simplification `fix/notes-typing` may want.
- **plans/html_core.md:** Sheet as CSS scroll snap, and which on-screen keyboard the web gets. Both need a design
  decision.
- **plans/search_library.md:** fuzzy name matching (no design yet) and more types.
- **plans/notifications.md:** the web subscription, Android, and Firebase credentials.
- **plans/deferred/large_blocks_lazy_rekeying.md:** deferred by the user (the forged-escrow problem).

## Known problems

- **Flaky host tests:** several fail under load besides those above. The notes-typing test is the worst.
- **Build server:** the scheduler is small (2 vCPU, 4 GB, swapping), and the daily VM-start limit capped the workers
  at 2, so builds are slower than they should be. A bigger scheduler is planned.
