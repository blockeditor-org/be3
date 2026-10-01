# Daily driver

A plan to get the app to the point where the user keeps their own notes in it.
Each phase lists what "done" looks like.

Real notes live in one remote workspace on a server running on a single VPS.
Local-only workspaces (the embedded server) stay, but only for testing, so
nothing here has to protect their data. Offline support on web waits until
before a public release.

From the freeze on (phase 8):
- Data written by the app must keep working in later versions.
  - That holds for the text and folder blocks and everything they sit on:
    objects, commits, metadata, keys and the server's database.
  - Other block types may keep breaking.
- Deleting the client or server database stops being an acceptable fix.

Formats can keep changing until the freeze, which is the last step (phase 8).
Phase 2 makes them versioned so the freeze is cheap when it comes, but nothing
is held fixed before then.

## Phase 1: Stop silent data loss (done)

Landed. What it left open:
- The server changes its in-memory object refcounts before the transaction
  commits. A failed commit leaves them out of step with the database until
  a restart.
- A conflicted merge keeps the unsaved side as a commit (`Peer::stash`),
  but the server does not count that commit's objects as held. Phase 7 must
  hold them before any collection runs.
- A corrupt object on the server is only noticed when it is read, because
  `has` checks that the file exists, not that it is intact.
- Conflicts and diverged sessions are reported only through the status
  error and the debug window. Phase 5 adds the UI.

## Phase 2: Version the formats

This is format work, not the freeze: it can land at any point, and the formats
it versions can still change afterwards. Nothing written today carries a
version. Everything is bare postcard, which
cannot tell a missing field from corruption, and whose enum variants are
positional.

### What gets a version

- **Sealed objects.**
  - Add a one-byte format/algorithm prefix before `nonce || ciphertext`
    (`be-store/src/vault.rs:38-62`).
  - A future cipher, nonce scheme or key epoch can then be told apart from
    corruption.
  - The prefix also carries the key epoch (phase 3).
- **`Commit`, `Manifest`, `be_vcs::Tree`.**
  - `CommitId` is the hash of their exact bytes, so they must never be
    re-encoded.
  - Wrap each in a versioned enum (`enum CommitV { V1(Commit) }`), so a new
    shape is a new variant and old commits still decode.
- **`BlockMetadata`.** Version it the same way.
- **Streamed headers** (`TextHeader`, every blob header; `be-block/src/streamed.rs`).
  - Put a version before the postcard header.
  - Text stays raw UTF-8 after the header. That part is already the right
    long-term format.
- **be-model documents** (folder, settings, profile and the rest).
  - Prefix `Tree::encode` with a magic and a version.
  - Freeze the order of the `Value` and `Change` variants: append only, and
    say so in a test.
  - `Tree::upgrade` replaces a field whose kind changed with a blank, and
    that blank is written out on the next seal (`be-model/src/tree.rs:47-60`).
    Make a kind mismatch keep the stored value and refuse writes to it.
  - `merge_fields` drops trailing fields that only the other side has
    (`merge.rs:149-182`). Keep them.
- **Wire protocol.**
  - Send `PROTOCOL_VERSION` (`be-protocol/src/lib.rs:9`, never read today)
    in `Login`, `Register` and `Authenticate`. Refuse a mismatch with a clear
    "update the app" error.
  - When a frame does not decode, reply with its request id.
- **SQLite.**
  - The server (`be-server/src/schema.rs`) and the client
    (`block-app/src/app_state/native.rs`) both use `CREATE TABLE IF NOT EXISTS`
    with no migrations.
  - Done: `PRAGMA user_version`, ordered migrations, and a test that opens
    every older schema.
  - Web `SavedAccount` fields get `#[serde(default)]`.

## Phase 3: A real encryption key, with backup (done)

Landed: random workspace keys kept on each device, a recovery phrase per account
whose public half seals every workspace key on the server, unlocking a new
device with the phrase or a code typed on an open device (SPAKE2 over the
server's relay), and sealing for invited members. `guides/the_new_block_stack.md`
(Keys) describes it.

It differs from the first design in one way: there are no device key pairs.
Each device keeps the workspace key itself and the server holds only copies
sealed to recovery keys, because a device key pair added nothing a device's
own copy does not already give it, and pairing hands the key over directly.
What it leaves for a public release is in `plans/public_release.md`.


## Phase 4: A server you can leave running (done)

Landed:
- Registration is closed unless `--allow-registration` is given, and
  `--add-account` reads the password from standard input.
- Password checks run off the database lock, at most four at a time, with a
  dummy hash for unknown emails. An email is locked out after five failed
  sign-ins, for 30 seconds doubling to an hour.
- A handshake timeout, server pings with an idle timeout, a bounded outbound
  queue, a connection cap, and accept errors that no longer stop the server.
- `tracing` logs, and SIGTERM stops the server after a WAL checkpoint.
- `be-server backup`, `restore` and `verify`, to a directory or a Bunny Storage
  zone, with the database sealed by a backup key and old snapshots thinned.
- systemd units, a Caddyfile and `guides/hosting.md`.

Garbage collection stays off: the app never sends `CollectDetached` or
`PruneHistory`, and unpublished objects are never collected, until trash and
history (phase 7) define what may be reclaimed. Token expiry, changing a
password, per-IP limits, storage quotas, a health endpoint and a config file
wait for a public release (`plans/public_release.md`).

## Phase 5: Offline and flaky connections

Today:
- a cold start without network cannot open a remote workspace (`main.rs:668-716`);
- heads and the graph live only in memory (`peer.rs:112`), so cached objects
  cannot be read offline;
- unsealed operations are dropped on disconnect;
- queued commands live only in an in-memory channel;
- no request has a timeout and no side sends pings, so a half-open socket
  hangs the worker;
- the status bar can say "All changes saved" while offline.

### Local-first client

- **A local refs database** beside `be-objects` (SQLite on native) holds:
  - each block's local head and last-known remote head;
  - the graph, with its metadata;
  - the workspace list;
  - the outbox of graph operations that are not yet acknowledged.
- **Opening a block reads the local head first.** Joining the session and
  fetching the remote head happen in the background.
- **Editing offline seals local commits.**
  - `commits.write` without publish.
  - On reconnect, `be_session::resume` / `Live::reconcile` decides between
    fast-forward, publish and merge, which is the machinery that already
    exists.
- **Cold start offline.** Open `last_workspace_id` from the local database,
  and show workspaces from cache when the list request fails.

### Connection

- Add a request timeout.
- Client pings every ~15 s; a missing pong marks the connection lost.
- Reconnect with exponential backoff and jitter, plus an immediate retry on
  OS network-change, foreground and focus events.
- Send lease heartbeats on a timer, not only on seal.

### Durability

- Seal locally on Android `onPause` and on desktop backgrounding, not only on
  Destroy.
- `status().unsealed` counts the outbox and local-only commits, so the
  close and account-switch guard tells the truth.

### Status UI

- The status bar shows one of: offline (n changes waiting), syncing, saved,
  or error.
- A conflicted merge (from phase 1) shows on the block, with a way to see
  both versions.

### Testing

- A test transport that drops, stalls and half-opens connections on demand.
- Tests:
  - edit offline, restart, then reconnect;
  - two devices edit the same note offline, then both reconnect;
  - the connection dies mid-seal.
- No sleeping: drive time with the frame clock and the transport.

## Phase 6: Text editor at 120 fps on a large file

**Target:** a 10 MB / 100k-line markdown file. Typing, caret moves, clicks and
scrolling each stay within an 8 ms frame, and a focused, idle editor stops
waking the GPU.

Today scrolling is mostly fine: rows are virtualized and painting is
retained. Editing is O(document) many times per keystroke. Fix in this order;
each item has a work-count test in the style of
`a_long_text_area_only_builds_the_lines_in_view.rs`.

**Profile before guessing.** The list below comes from reading the code. Before
working through items 3-8, profile the real app with a document of 10,000
lines of 200 words each (about 12 MB):
- Generate it as a text block, open it in the native app, and also measure on
  Android.
- Record a CPU profile (`perf` with frame pointers on Linux, or the
  `performance` module's frame timings) for each of: typing a character,
  holding an arrow key, clicking, scrolling a page, scrolling to the end, and
  sitting idle with the caret blinking.
- Note the p50 and p99 frame times and the top functions for each, in the
  plan or the PR, and re-order the items below by what the profile shows.
- Profile again after each item lands.

1. **Anchors (done).** Anchors exist only for positions in use
   (`text_editor_core::AnchorTable`), so finding one no longer scans the text.
   The bytes are still a flat `Vec<u8>`, spliced in O(n) at several layers. A
   rope or piece table is worth it once items 3-5 stop the whole-document
   copies, and measuring says the splice matters.
2. **Graphemes (done).** Boundaries are found within the surrounding line, and
   an ASCII byte on the right needs no segmenting at all.
3. **Measuring.**
   - `TextAreaState::measure` shapes every line on each change, which also
     empties the shape cache (`text_area/state.rs:413-449`).
   - `table_spacers` does the same for tables.
   - Done: cache height per line and invalidate only the edited lines.
4. **Highlighting.**
   - The tree-sitter parse is incremental, but the style pass builds a style
     per byte and walks the whole tree on every edit
     (`highlighter.rs:230-327`).
   - Done: highlight lazily per visible line, from the tree's changed ranges.
   - `collapsible_sections()` runs on every caret move (`state.rs:549-552`).
     Run it only on content change, and incrementally.
5. **Whole-document copies on every edit:**
   - `read_bytes`;
   - the `described` and `shown` strings, plus their equality compares;
   - `line_starts`;
   - the `VirtualList` keys;
   - the checkbox scan;
   - `parse_embeds`;
   - `adopt`'s full compare;
   - the host's `session.bytes()` re-encode in `publish()` (`be/worker.rs:1166`).
   Each becomes incremental or is dropped.
6. **Sealing.**
   - Sealing every 750 ms re-encodes, re-chunks, re-hashes and re-encrypts
     the whole document.
   - Done: debounce sealing on idle, with a max interval. Reuse the chunks
     before the first edited byte, since content-defined chunking
     resynchronises after the edit.
7. **Scrolling in plugins.** It repaints the whole viewport, because the
   plugin path unions the moved region into its damage
   (`block-editor-beui/src/instance.rs:83-96`). Carry the native presenter's
   GPU move copy across the plugin surface.
8. **Idle.**
   - Stop the caret blink after a few seconds without input, as most editors
     do.
   - Finish the "Push, don't poll" backlog item.
   - Done when a test confirms a focused, idle editor requests no frames.

Add a frame-time benchmark that types into a generated 10 MB markdown file
through the plugin host and prints p50 and p99 frame times. Run it in CI as a
work-count budget rather than a timing assertion.

## Phase 7: The notes app itself

Needed before daily use:

- **Move to… on touch.**
  - A folder picker sheet from the ⋮ menu on phone and from the context menu
    on desktop.
  - Today phone has only "Move to the top level" (`file-tree/src/app/phone.rs:776-781`).
  - Also add Duplicate.
- **Search.**
  - A `BlockQuery::Search` that searches names and text content on the
    client, over the local cache. The server cannot read content.
  - Wire it to the phone search field, which is a placeholder today
    (`phone.rs:293-303`), and to a desktop quick-open (Ctrl+P / Ctrl+K).
- **History and restore.**
  - A per-note version list built from the commit chain, with preview and
    "restore this version" (a new commit, not a rewind).
  - Bookmarks (`CommitKind::Bookmark`) are kept by retention.
- **Trash.**
  - Remember the original parent, so restore goes back there.
  - Add "Delete forever" and "Empty trash".
  - Any automatic purge waits until history and backups are trusted.
- **Export and import.**
  - Export a folder or the whole workspace to a directory of `.md` files,
    with links between notes rewritten as relative paths.
  - Import a directory of `.md` files.
  - Being able to leave is part of trusting the app with notes, and this is
    also a second, plaintext backup.
- **Created and modified times** on `BlockInfo`, for sorting and recents.
- **Linking.** `[[` autocomplete to link a note while typing.

Nice to have later:
- daily-note action;
- heading level picker;
- image button that picks a file;
- trimming the Add picker to the block types that are ready.

## Phase 8: Freeze the formats

The last step, once the formats have settled through real use. From here on,
text and folder blocks and everything under them keep working across versions,
and deleting a database stops being a fix.

- **Golden fixtures.**
  - Check in bytes that the current code writes: a sealed text block, a
    folder, metadata, a commit, a snapshot tree, a server database, and an
    app-state database.
  - The fixtures live under `snapshots/formats/`, and `//:verify` decodes
    them.
  - A test fails if a frozen type's encoding of a known value changes.
  - This turns "keep text and folder working" from a promise into a check.
- **A short format section in `guides/the_new_block_stack.md`.** It names
  what is frozen and how to evolve it: new variant, never edit one.

## Suggested order

1. Start the server on the VPS (`guides/hosting.md`), with backups from day one.
2. The profiling at the start of phase 6.
3. Phase 2, versioning the formats, whenever it is convenient.
4. Phases 5 and 7 in parallel. Start real use when offline open, the status
   indicator, Move to…, search and export exist.
5. The rest of phase 6, measured against the profile.
6. Phase 8, the freeze, last.
