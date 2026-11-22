# Collected reviewed work (rebase-merge)

The description of [blockeditor-org/be3#1015](https://github.com/blockeditor-org/be3/pull/1015), kept here so it can be edited like any other file. Delete this file when the pull request merges.

One branch that collects reviewed work, so it lands together instead of as many small pull requests that conflict while CI waits. The commit list shows what is included. Each piece is one or a few commits on a linear history; merge with **rebase** to keep it.

Nothing is added until it has passed review and `./scripts/verify` has passed on the exact commit the branch moves to. The branch is treated like main: rebased onto main when main moves, and force-pushed only for that.

## Open work (2026-11-22, paused)

Development is paused, and no agent is working. Nothing below is on this branch yet. Each item says where it stopped, so it can be picked up later. Branches are on origin.

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
- block-rows-guard: also guard the other columns that name a block (`links.grant_block`, `access_log_authorities.above_id`, `block_edges.reference_id`, `blocks.parent_id`), or document that `forget_blocks` takes whole subtrees made by an account that holds no links.

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
- `claude/chat-stall` @ 74e28811 (on b5d4bcfa): the §15 chat stall.
  - Cause: a fresh chat editor's log read was in flight when the host reopened the chat's replica, and be-sdk-type-guest's `answered` drops every answer to an earlier epoch, so the editor waited forever.
  - Fix: a replica's unanswered editor log reads carry across a reopen (`Carried::log_reads`, `lacking::EditorRead`), and the new replica asks for them again in `start`.
  - The new test `an_editors_log_read_is_answered_though_its_replica_reopens_before_the_answer` fails without the fix. Stress went from 3 failures in 96 to 0 in 1,000.
  - guides/type_modules.md says what a reopen carries, and plans/backlog.md §15 is replaced.
  - Same class, not fixed: an editor's in-flight part put or append is dropped the same way on a reopen. Asking again is safe for a put, but an append could land twice.
  - Flaky on the tip at the same rate with or without this fix, now in §15:
    - `a_seal_whose_publish_was_lost_trims_the_journal_when_the_outbox_drains_it`, about 1 in 12;
    - `a_joined_channel_opens_to_its_host_and_again_to_the_next`, about 1 in 2.5 at 16 at once. acceptance-2 fixes the latter.
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
- `claude/flush-after-revoke` @ 78176b42 (one `wip:` commit on b5d4bcfa): fixing `a_publish_right_after_a_revoke_lands_without_waiting`, which stalls only under verify's load. That test is out of verify since 5b, so it doesn't run now.
  - When it stalls, the status reads "owes a seal: true, publishes unanswered: 0", so no seal was tried. The module skipped it: the replica wasn't the lease holder, nothing was owed yet, or it was reloading. These tests' clock never moves the quiet-spell seal, so a skipped seal is never redone.
  - The commit holds:
    - the test, moved back from 5b;
    - the stall report, which now shows whether the replica holds the lease;
    - an unconfirmed candidate fix: `Served` notes a flush that ran while commands waited their turn, and the worker seals again once they have run (`take_flushed_ahead` → `seal_now`);
    - an attempt at a deterministic test, which passes even without the fix and so doesn't reproduce the stall.
  - Left:
    - Catch one failure under load with the lease field shown.
    - Make it deterministic by holding the lease move or the rejoin, not the lane.
    - Fix the source, so a seal skipped for a reason that later clears is asked for again. Then drop whichever of the attempt test and the `flushed_ahead` change doesn't hold up.
    - Add a backlog line pointing at the branch.
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

Some of these apply only once the piece named is on this branch; check the commit list.

**Deploy workflow (once "deploy from a manually dispatched workflow" is included), in order:**
- [ ] Generate an ed25519 SSH key used only for deploys. Uncomment the "CI deploy key" entry in `servers/common.nix` with its public half, land that, and deploy servers/api by hand (guides/servers.md). The workflow can't install its own key.
- [ ] Create the GitHub environment `deploy`. Any branch may deploy, so required reviewers are its only gate (agents with the actions trigger tool can dispatch). Add the secrets:
  - `BE3_DEPLOY_BUNNY_STORAGE_KEY` (the be-app storage zone password)
  - `BE3_DEPLOY_BUNNY_API_KEY`
  - `BE3_ANDROID_RELEASE_KEYSTORE` (the keystore's base64; without it, untick the APK)
  - `BE3_DEPLOY_SSH_KEY` (the new private key)
  - `BE3_DEPLOY_KNOWN_HOSTS` (`ssh-keyscan api.be3.pfg.pw`, checked against the real host key)
  - `BE3_NL_TOKEN` (a repository secret is enough if one exists)
- [ ] Bunny pull zone:
  - send `Service-Worker-Allowed: /` on `/data/*`. Without it the first deploy stops before the server swap, after uploading be and the downloads;
  - give `/`, `/index.html` and `/live/*` a one-hour TTL and `Cache-Control: no-cache` to browsers;
  - optional: `/data/*` serves `max-age=2592000` today; guides/hosting.md expects one year, `immutable`.
- [ ] Optional: delete the stale `/web/downloads.html` from the zone (the downloads page moves to `/live/downloads.html`).
- [ ] Expect `./scripts/be`, setup's be step (it only warns now) and the Tracker workflow to fail until the first successful deploy writes `live/be-release`.
- [ ] After the first deploy, load https://be3.pfg.pw once and check the service worker's scope is `/` with a script under `/data/BUILD/web/sw.js`, and that an existing push subscription survived.

**Accounts (once "passwords go" is included):**
- [ ] After deploying, nobody can sign in with a password. Existing accounts sign in by emailed code, and the hosted server delivers mail only to its log (journal). Make a login link with `be-server login-link --auth-origin https://api.be3.pfg.pw --email YOU` on the server (guides/hosting.md); it also makes a new server's first account.
- [ ] Remember that whoever can read the server's journal can read every sign-in code. Real mail delivery is deferred (plans/deferred/mail_delivery.md).
- [ ] Once "invitations bound to no email" is included: before deploying it, the hosted server's `links` table must lose two columns, or be-server refuses to start on its schema check. Stop be-server and run `sqlite3 <data-dir>/<server db> 'ALTER TABLE links DROP COLUMN invite_email; ALTER TABLE links DROP COLUMN invite_role;'`, or reset the links table. Outstanding invitations made before then are bound to an email and should be remade.
- [ ] Once "approving a device's code needs a fresh proof" is included: after deploying, existing app sessions have no expiry and stop working, so every device signs in again once. Approving a be-host sign-in asks for a fresh sign-in, and app tokens expire after 90 days unused.
- [ ] Recovering an account (by code or an operator login link) now removes its other ways in (passkeys, providers) and its agents, and says what went. `be-server login-link --account ID` also removes the account's email, since it vouches for none; use `--email` to keep an address you've checked.

**Google / provider sign-in (once "OIDC" is included):**
- [ ] For each provider, register a web application with redirect URI `https://api.be3.pfg.pw/auth/oidc/ID/callback` (ID is the provider's id in the file below). Google: an OAuth client of type "Web application" in the Google Cloud console; the server asks for openid, email and profile.
- [ ] Write the providers file next to the data directory, readable only by be-server: `[{"id":"google","preset":"google","client_id":"…","client_secret":"…"}]` (format, and single-tenant organization providers, in guides/hosting.md). `authoritative_email` stays false except for providers whose verified emails you trust.
- [ ] Add `--oidc-providers FILE` to the be-server NixOS service in servers/ and deploy. It refuses to start on a malformed file; without the flag, no provider buttons show.
- [ ] Changing `--auth-origin` later changes every redirect URI. Removing a provider strands accounts that sign in only with it; the server warns how many, and `be-server login-link --account ID` lets one in.

**Android push (once "Android receives pushes through UnifiedPush" is included):**
- [ ] On a phone, install a UnifiedPush distributor. Sunup works with the server as-is; ntfy needs `--push-host ntfy.sh` (or your own ntfy host) added to the be-server service in servers/ and deployed.
- [ ] Run the device checks listed in guides/running/android.md ("Push notifications"); nothing on the Java side could be run here (no emulator).

**Local accounts (once "be-host makes local accounts on the device" is included):**
- [ ] be-host no longer starts the embedded server, so a local profile made with it no longer opens (no migration). Remove it and create a new local account; the old data in the data directory's `server/` folder can be deleted. Local accounts now run with no server, also on the web, where they live only in that browser's storage.
- [ ] The server's create-local-profile call and local_profiles setting are gone. An existing server database keeps an unused `accounts.local_profile` column: harmless, or drop it by hand.

**Data (once "#686" offline editing is included):**
- [ ] The session wire protocol, type API, device refs tables and journals changed with no migration: delete existing device data directories and the server's data before running this build. The server now orders live sessions; "Unsaved" counts edits this device does not yet keep.

**Signed access (once "signed access 3b" and "3a" are included):**
- [ ] The protocol and server schema changed: set-access and set-role take a signed record; create-agent takes a signed bound-to and its account id is derived from its maker and keys; a new `sign-member`; `remove-member` takes a member record; escrow keys gain a maker and escrow wraps change format; new tables `keys.maker` and `*_log_names`. Update the server and every client together and reset the server's data, since existing escrow keys, wraps and access logs won't check.
- [ ] Discarded edits: a join's answer now says whether the account may edit (`may-edit` in the `joined` record). A discarded block's edits stay discarded and its editors are read-only until the server says it may be edited again, and a rejoining replica no longer submits an edit twice. Covered by the same data reset.
- [ ] Contacts: a device now pins each contact's newest key and shows contacts by the name on the card it verified, not the server's. Contacts made before this read as not confirmed until connected again, and a contact's rename shows only after they connect again.

**Signed access 3c (once "opening a kind to anyone inside is a signed record" is included):**
- [ ] The protocol gains `removal-revocations` and key work `unsigned-revocations`; remove-member and remove-contact carry signed revocations. A kind opened to everyone inside before this has no `Record::Opening`, so it offers nothing to contacts until it is reopened (no migration). Who counts as inside is still the server's tree until signed access step 4.

**Crypto audit (once "one symmetric seal" is included):**
- [ ] One AEAD seal format binds its header and its key's block (HIGH 3); a key the server holds seals only the settings block (HIGH 2); an owner-only block's data keys are sealed to nobody and never escrowed (item 6); a backup's seal binds its snapshot name (item 14). Reset data, since the seal format changed. Backups made before this (`be3-backup-v1`) don't restore; a restore logs the snapshot's time.

**Wasm on V8 (once "wasm on V8" is included):**
- [ ] Every wasm guest outside the browser now runs on V8 (rusty_v8 152.2.0) instead of wasmtime, and nothing is precompiled: a release bundle went from 1,235 MB to 497 MB, and be-host's binary from 162 to 232 MB. Existing devices keep a stale `<data_dir>/app-cache`; delete it by hand.
- [ ] Guests may no longer declare wasm GC types or more than one table (V8 can't stop an isolate whose heap is held at its limit).
- [ ] Starts cost more: across 141 host tests CPU went from 397 s to 756 s (about 1.9×) and the median start to the Files pane from 0.56 s to 1.67 s (about 3×). Most of what was recovered came from trimming each library's exports to what the staged apps import; an on-device code cache (TurboFan code, checked before use) helps later starts. Host tests read a cache their own build makes; it is test-only and never shipped.
- [ ] Android builds again on our own rusty_v8 152.2.0 build for aarch64-linux-android, pinned in third-party/v8 (hosted on lfs.pfg.pw) and built by `scripts/internal/build-rusty-v8-android.sh`. The debug APK links. Install it on a device and open an app to confirm guests run; a clean rerun of the script is also unconfirmed.

**Windows space mode (once "Windows opens no window until it is picked" is included):**
- [ ] Only compiled here. On Windows, run `BEUI_LAYERS=space ./scripts/buck run //crates/be-host:native -- --desktop` and check: no window is captured until picked from a menu; a grab-handle click doesn't focus or lock; only a click in content focuses and locks the mouse; Ctrl+Alt+Space defocuses.

**Headless apps in `be` (once "be runs an app's host component headless" is included):**
- [ ] A deployed `be` downloads apps from its release, checked against the app-hashes.json its deploy-written build-info.json pins; only releases deployed after this carry `apps`. Covered here only by fakes, so check `be` against the first real deploy. Since step 1d, `be` also downloads app manifests and type modules from its release (all type modules on its first command that reads content, then cached). A development terminal host needs `~/.cache/be3/terminal-host/ARCH` rebuilt from `//crates/be-tool-cli:terminal-host` (the error names the command).

**Text (once "a text area's newline marker room is a Blank" is included):**
- [ ] Only Chromium was run. Check a line just wide enough in a text area in Firefox and Safari: its last word stays, and a selected newline highlights at full height. Natively the hung ⏎ is partly clipped at an exact fit.

**Mailbox and new phrases (once "the mailbox" and "a new phrase reseals what the account sealed to the server" are included):**
- [ ] Account notices, pending key changes and accepted links' cards move into one mailbox; the `account_notices` and `key_changes` tables and `links.accepter_card` go. Reset the server's data and every device's refs database, and delete old logs.
- [ ] Setting a new recovery phrase now reseals, on the server, what the account had sealed to it, so an administrator's new phrase no longer leaves the organization's policies unreadable.

**Known behaviour:**
- [ ] On a server with no link base (a local server), Copy's plain text carries the block as a `#block/<org>/<id>` fragment rather than a full link.

**Build server:**
- [ ] Release builds no longer use LTO (decisions/no_lto.md); the build server's swap comment moved to servers/build/default.nix.
- [ ] The build server is the NixOS configuration `servers/build` (NativeLink 1.8.6), already deployed from #1016, which this branch now carries.
- [ ] Host tests sometimes fail under a full verify because an app's step starves for 10 s ("The app stopped responding"; 7.1 s system CPU against 0.3 s on a passing run), which looks like memory pressure on a crowded NativeLink worker. Cap how many host tests a worker runs at once by memory, not only CPU. Host tests now fail at once naming the stopped app.
- [ ] Around 06:40–06:57 on 2026-11-16 memory reached 28 GB plus 16 GB of swap, then builds kept failing with "missing from CAS". No release, LTO, deploy or Android build ran; the dashboard's per-action peak memory for that window should name the action.

**Repository:**
- [ ] CI is disabled; re-enable the CI workflow when you want it back. This branch changes crates/be-tool-bootstrap, and only CI publishes and pins a new bootstrap (guides/bootstrap.md), so local `./scripts/*` keep using the old pinned bootstrap until CI runs on this branch.
- [ ] Delete the stale branches `claude/headless-c-7`, `claude/headless-c-7-rebased`, `feat/menu-intent-typeahead` and `feat/menu-intent-typeahead-2` to `-4`, and other numbered branches whose last number landed.
- [ ] Delete the stale branch `fix/stack-test-flakes` (its work merged from `fix/stack-test-flakes-rebased`).
- [ ] Optional: AGENTS.md's principle "No in-app reach to strangers" could mention that a share link also counts as an out-of-band start (decisions/share_links_are_an_out_of_band_start.md, once included). Agents don't edit that list.
- [ ] Delete this file.
- [ ] Merge this pull request with **rebase**.
