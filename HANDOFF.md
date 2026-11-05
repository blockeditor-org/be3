# Handoff: large blocks and saving live edits less often

Delete this file before opening the pull request.

## The task

The user asked for plans/large_blocks.md and plans/live_edit_persistence.md
(from #790, now on main) to be implemented in one PR, with the plan files
deleted once done. Both are implemented on branch `ccr-bcde3032-s9gm0f`, both
plan files are deleted, and main (with #790 and #797) is merged in. What is
left is verification, one flaky-painting fix, and the PR.

## What is on the branch

- **Large blocks (all of it):**
  - Content chooses its parts. `BlockContent::parts` and `from_stored` use the
    `Part`, `PartRef` and `Stored` types in be-block-store. `Blob`'s header is
    a part of its own.
  - The manifest is a tree of sealed nodes.
  - `publish` sends object and reference changes (be-block-protocol), and the
    server writes only the edges that changed.
  - Each part carries its own references.
  - be-block-model splits documents into parts deterministically
    (`DocumentPart`, crates/be-block-model/src/parts.rs). Parts cross the type
    protocol as `written.parts`.
  - A type can load only its first part (`Loads::FirstPart`), with `load` and
    `read-part` host calls and editor range reads (`LiveContent::read_range`).
    Every `Blob` does this, and blob editors use `ContentProjection::payload`.
  - A write under a new data key reseals reused parts from older keys
    (`Vault::reseal`, `stale_parts`). This fixed a security bug where taking a
    share back left the head readable.
  - Leftovers: the part-hash check moved to plans/chat.md, and loading part of
    a document is item 6 of plans/backlog.md.
- **Live edits (all of it):**
  - A journal on the server (`be_server::journal`, `sessions.append` and
    `rebase-journal`) and on the device (refs DB).
  - Commits at pauses (`QUIET_INTERVAL`, `COMMIT_INTERVAL`, `JOURNAL_BYTES`,
    `APPEND_INTERVAL` in be-sdk-type-guest replica.rs). New owners and joiners
    recover from the journal. Readers outside the session replay it
    (`Modules::readable`).
  - Pruning at publish (`ServerStore::thin_history`, one state per minute for
    the last hour). The `prune-history` call and `Peer::prune` are removed.
    Loading a commit is deterministic for every replica.
  - The journal is tied to the module tag. A lost journal marks the block on
    every device, and the editor toasts about it.
  - Session restarts, triggered by cost. `LiveEdit::carry_over` carries every
    kind of pending edit over. Undo stops at a restart, with "Can't undo past
    here" and a Version history button.
  - Decision record:
    decisions/sessions_restart_when_their_state_outgrows_the_content.md. It
    also lists the limits that were accepted.
- **Bugs found and fixed along the way:**
  - The History tab's Current row raced with pruning; `history-entry.head` now
    marks it.
  - Restore keeps the state it replaces: a bookmark on the owner, a merge
    parent on a follower.
  - Retention handles same-millisecond ties and `keep_all_within` 0.
  - `be_host_stack::wait_until` fails after 60 s instead of hanging the target.
- **Test clock:** `be_block_commit::pin_now`, pinned per frame by be-host when
  beui's wall clock is pinned. Host tests can move history on with
  `pause(60_000)`.

## Verification state

- The last full `./scripts/verify` pass ("All checks passed") was on 88c12322,
  which had everything up to session restarts.
- Unverified since then:
  - e3eb8442: carry-over of every edit, the restart ceiling, the lost mark on
    every device, deleting plans/live_edit_persistence.md.
  - 2efbdc25: restore on a follower, the retention fix.
  - aedd2b62: deleting plans/large_blocks.md.
  - The merge with main (#797, dock bars; it touches no shared files except
    plans/).
- All of those passed their crates' tests under local cargo.

## How to continue

1. Run `./scripts/verify`. Both build servers were down at handoff: Namespace's
   waker and `BE3_BUILD_SERVER=blocks.pfg.pw` both answered 502. Use whichever
   works; `BE3_BUILD_SERVER=blocks.pfg.pw` worked earlier in the day. Run it in
   the background with a 45-minute timeout and fix what fails. If the
   be-host-stack target times out, `wait_until` now names the stuck test.
2. Fix the flaky be-host-tests painting
   `a_popup_in_space_closes_on_a_press_in_another_panel`. It flips between 935
   shapes (no hover) and 1001 shapes (the "New in My files" row hovered, with
   a hand cursor).
   - The diagnosis is not yet confirmed by a run. On a fresh be-host
     (`BEUI_LAYERS=space`), a child app view's first open doesn't see the
     resting pointer. beui's point_again/reforward sends it to the new sink
     and records it as hovered. But be-host-runtime `Instances::forward`
     (crates/be-host-runtime/src/instances.rs, around line 1365) drops input
     for a region with no screen yet (`entry.screens.get_mut(&region)` is
     None), and nothing sends it again.
   - Fix: hold the pointer state forwarded to a region without a screen, and
     deliver it when the screen is created or mounted (`Instances::show` /
     `mount`). Add a test, and accept the painting with the hover.
3. Delete this file, run verify again, and open the PR from
   `ccr-bcde3032-s9gm0f` to main. Use the PR template headings if there is one.
   The PR body is below. AGENTS.md says not to watch or subscribe to the PR.

## Notes

- Local cargo works for most crates while the build server is down. For
  be-host-stack and be-tool-cli, first run
  `bash scripts/internal/build-ghostty-vt.sh --triple x86_64-unknown-linux-gnu`;
  be-tool-cli also needs `APP_TYPES` and `EDITOR_MANIFESTS` set. The
  be-app-workspace paintings `a_phone_shows_one_file…` and `the_rename_card…`
  fail only under local cargo (font rendering); they pass under buck.
- Records containing `option<commit-id>` (such as `undo-state`) in
  be-sdk-type-api's bindgen `default` list made rustc crash (an internal
  compiler error) under buck. Keep them out of that list.
- The permission classifier refuses force pushes and remote branch deletes.
  Stale remote branches can be deleted by hand: `lb/chunks`,
  `lb/model-parts`, `lb/model-parts-wire`, `lb/model-parts-rebased`,
  `lb/journal`, `lb/prune`, `lb/publish`, `lb/partial`, `lb/blob-partial`,
  `lb/rekey`, `lb/verify`, `lb/history-fix`, `lb/restarts`,
  `lb/restart-gaps`, `lb/followups`.

## Draft PR body

**Title:** feat: large blocks and saving live edits less often

Implements plans/large_blocks.md and plans/live_edit_persistence.md (#790) and
deletes both. What is left moved to plans/chat.md (an operation that adds a
part carries its hash), plans/backlog.md (loading part of a document) and
decisions/sessions_restart_when_their_state_outgrows_the_content.md.

- **A save costs what changed.**
  - Content returns its stored form as parts, documents choose parts from
    their structure, and the manifest is a tree.
  - Publishing sends object and reference changes, and each part carries its
    own references.
  - Typing in a 1 MiB text block uploads one part and a few nodes.
- **Partial loading.**
  - A type can load only its first part, and editors read the rest by range.
  - Every blob (image, audio, PDF, game module, file) loads only its header.
- **A journal instead of a commit every 750 ms.**
  - Edits are durable in a journal on the server and the device.
  - Commits come after 30 s quiet, 5 min after the first edit, at 1 MiB of
    journal, or on close.
  - Readers outside the session replay the journal.
- **Pruning at publish**, keeping one state per minute for the last hour.
  `prune-history` is gone.
- **Session restarts** when session state outgrows the content, carrying
  pending edits and cursors over. Undo says "Can't undo past here" and offers
  version history.
- **The journal is tied to the module version.** Edits lost to a version
  change are marked on every device and shown in the editor.
- **Fixes:**
  - Taking a share back now reseals reused parts under the new key.
  - Restore keeps the state it replaces.
  - The History tab's Current row no longer races with pruning.
  - A stuck host-stack wait fails instead of hanging the suite.
