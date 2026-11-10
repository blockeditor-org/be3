# Work status (paused 2026-11-10 ~02:20 UTC)

All agents are stopped. Every in-progress branch is pushed to origin. Work that was not committed when the agents
stopped is committed as `wip:` (unverified). Nothing below has merged unless it says so. This file is not meant to
be merged: delete the branch `docs/work-status` when the work is picked up.

## How the work was run, and how to continue it

- **Process.** A coordinator agent started builder agents, one per plan or fix, each in its own git worktree
  (`/home/user/wt/<task>` on the VM `agents`). A separate reviewer agent read each pushed branch (read-only) and wrote
  findings to `/home/user/.cache/be3-slot/review-*.md`. The builder then fixed them, and the cycle repeated until a
  review found nothing blocking. Reviewed branches were batched into one PR at a time with auto-merge, since CI
  takes ~15 minutes.
- **Builder instructions:** `/home/user/.cache/be3-slot/PROTOCOL.md` (the UPDATE section at its top is current).
  Builds go through `/home/user/bin/be3-remote WORKTREE -- COMMAND`, which:
  - runs it on the NativeLink build server (`BE3_BUILD_SERVER=nativelink BE3_BOOTSTRAP=cargo`), at most two at once
    because of the VM's 15 GB of memory;
  - stops the worktree's buck2 daemon after each command;
  - falls back to the local build slot (`/home/user/bin/be3-slot`, local buck2 in `/home/user/be3`) when both places
    are busy or the server is down.
- **Build server access:** `~/.config/be3/nl-url` and `~/.config/be3/nl-key`. Memory note:
  `nativelink-from-boat-vm.md`.
- **Builder VMs.** Four small boat VMs ran agents with their own checkouts. They are interrupted, not deleted:
  - `be3-builder-1` (`bx_yahaw6nw`);
  - `be3-builder-2` (`bx_3nf9w89x`);
  - `be3-builder-3` (`bx_hddjd8e4`);
  - `be3-builder-4` (`bx_rm7yxubs`).

  Their instructions are `~/VM-PROTOCOL.md` and `~/TASK.md` on each, and their reports are `~/REPORT.md`. They have an
  8-hour lifetime from ~00:35 UTC. Delete them with `boat delete ID` once their branches are picked up.

## Open PR

- **#829** `feat/backlog-3`: backlog item 1 (push, don't poll) and the files pane's paste.
  - Reviewed twice and ready, but CI fails on `be-host-tests:typing_right_after_adding_notes_reaches_the_notes`. That
    is a real regression: it fails in every full verify with this branch.
  - The fix in progress is `fix/notes-typing`, below. Add its commit to #829 (or merge it into `feat/backlog-3`), then
    let CI rerun.

## Branches ready to merge (reviewed, nothing blocking, verify passed)

Batch these into the next PR(s) once #829 merges. Rebase onto origin/main and run one verify first.
- **`feat/html-core-2`:** plans/html_core.md items 1, 2, 4, 5, 6, 8 and 9 (TextInput selection, forms posting beui's
  fields, themed scrollbars, TreeNode text hooks, paste and back on HTML, removing beui-font-browser, no inspector on
  HTML), plus fixes for two `//crates/be-host:web` build errors.
  - Reviews: `review-html-core.md` and `review-html-core-2.md`.
  - Its follow-ups are on `feat/html-core-3` (below); they can come later.
- **`fix/nl-wake-on-redirect`:** two build server fixes.
  - The bootstrap now wakes a stopped scheduler: a stopped boat sandbox's port answers 302, which was taken as "up".
  - `new-scheduler.sh` makes the scheduler with `--no-auto-stop`: an 8-hour boat lifetime stopped it mid-build.
  - Verify passed; small enough to need no review.
- **`fix/host-palette-follow-ups-2`:** the command palette's review findings A–D (menu-pick window, pruning hidden
  views' actions, anchored views, window hosts followed by layer changes).
  - Reviewed (`review-palette-3.md`); verify passed.
  - It was rebased onto an older main, so rebase it again.

## Branches in progress (pushed; what is left)

| Branch | What it is | State | Next step |
|---|---|---|---|
| `fix/notes-typing` | Fix for #829's failing test: keys typed right after Add notes survive a late notes editor (`e1284eea9`, on `feat/backlog-3`) | Its last two test runs failed (exit 1); the full verify was cut off | Find why `typing_right_after_adding_notes_reaches_the_notes` still fails. Clue from the search work: the test loses typed text whenever the host-stack worker loop stalls (slow inline work), so the keyboard gift (#825) probably drops held keys when the child is late. Fix at the source; run the test many times under load. |
| `feat/chat-1c-2` | plans/chat.md 1c: unread counts (`ChatReads` per device and message), app documents start without the keyboard, keyboard per view, `Focus` after a restart and after a close | All findings of `review-chat-1c.md` fixed; the last commit is verify's fixes | Run one verify to confirm, then get a delta review of the fixes. Then it can merge on its own. |
| `feat/chat-1b-3` | plans/chat.md 1b: "Sending"/"Not sent" via `FollowLacking`/`Lacking` from the replica, on top of `feat/chat-1c-2` | Fixes for `review-chat-1b.md` made; the verify at 02:13 passed | Delta review against `review-chat-1b.md`, then merge after 1c. (`feat/chat-1b-2` on origin is an older push.) |
| `feat/notifications-4` | plans/notifications.md steps 1, 2 and 7 and the server half of 6: Web Push (RFC 8291/8292), FCM, `notify` host call, mute menu. `notifications-3` passed review (`review-notifications-2.md`); `-4` adds the decided push-host allowlist | WIP commit: the allowlist and the review-2 follow-ups (N1–N5) are in progress and unverified | Finish the allowlist: built-in push services, an operator `--push-host` flag, `decisions/push_endpoints_are_allowlisted.md`. Do N1–N5 of `review-notifications-2.md`, verify, then a delta review. The web subscription, Android and a Firebase project are left out (the plan says so). |
| `feat/search` | plans/search.md steps 1, 2 and 4: the index block and its access, indexing on seal, `search-watch`, hits in the files pane and quick open | WIP commit; the last full verify failed on `quick_open_finds_a_word_typed_in_a_text_and_opens_it` and on the notes-typing test | Verify, and fix the two tests. If notes typing still flakes, ask for search text from a task, not the worker loop. Not yet reviewed: needs a full review after it passes. Step 3 (`changed-since`) needs a durable per-block change counter on the server. |
| `feat/html-core-3` | Follow-ups on `feat/html-core-2`: a dirty flag for the back sync, a reload stranding an entry, a retraction racing a push | WIP commit, unverified | Finish, verify (and `//crates/be-host:web`), then a delta review. |
| `feat/text-oversized-edits` | plans/text.md step 3: oversized edits go by `put-part`; deletes plans/text.md | From VM 2. Targeted tests passed; full verify not run; the last small changes are uncompiled | Verify; then a full review (it changes the session protocol). |
| `feat/authorship` | plans/authorship.md: Ed25519 signing key, `get-signing-keys`, `sign`/`verify` host calls, chat messages signed | From VM 3. Core crates built and tested; the WIT, be-sdk, runtime and chat parts are uncompiled | Compile, run `a_message_shows_whether_its_author_signed_it`, then verify. A full security review is needed. It touches be-type-chat, so rebase after the chat branches. |
| `fix/review-follow-ups` | The host owns a cut link (`cut-from` in the paste answer); Cmd/Option chord labels on Apple; tests for the palette's split and close actions | From VM 4, based on `feat/backlog-3` (#829). Items 1–2 tested; item 3 and the wasm parts uncompiled | Run `./scripts/test the_palettes_`, verify, and write a .pr.md. Land it after #829. |
| `fix/flaky-host-tests` | Root causes for `a_new_image_asks_for_its_file_in_the_image_editors_own_dialog`, `the_storage_page_says_what_the_server_counts`, `the_members_dialog_lists_…` and the 258/259-shape `the_add_menu_opens_at_the_files_button_that_asked` painting | From VM 1, WIP commit (26 files), unverified, no report | Read the diff, finish, and show each test passing many runs under load. |

## Plans not started, or left with a decision needed

- **plans/api_simplification.md:** not started. Its 15 steps were waiting for the protocol-heavy branches above to
  land.
- **plans/chat.md 1d and 2:** 1d needs a type-side put-part hook; 2 needs notifications.
- **plans/backlog.md** items 2, 3 and 5, and most of 4:
  - item 2 (bar drops across layers) needs a decision on the dock state model;
  - item 3 needs a web host-test harness;
  - item 5 (lazy lists) needs a decision on what reading an unloaded part returns;
  - in item 4, Android and macOS need devices, and refs in clip items and rich text marks are large.
- **plans/html_core.md:** Sheet as CSS scroll snap, and the web on-screen keyboard. Both need a design decision.
- **plans/search_library.md:** fuzzy name matching (no design yet) and more types.
- **plans/notifications.md:** the web subscription and service worker, Android, and Firebase credentials.
- **plans/deferred/large_blocks_lazy_rekeying.md:** deferred by the user (forged-escrow problem).

## Known problems

- **Flaky host tests:** several fail under load, beyond those in `fix/flaky-host-tests`. The notes-typing test is the
  worst.
- **Build server:** the scheduler is small (2 vCPU, 4 GB, swapping) and the daily VM-start limit capped the workers at
  2, so it is slower than it should be. Bigger scheduler planned.
- **Review findings:** in `/home/user/.cache/be3-slot/` on the `agents` VM. Each branch above names its review.
