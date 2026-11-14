# The client: peer, connection and live sessions

Part of the block stack (guides/the_new_block_stack.md). `be-block-client` is
a peer: a local object store, a connection, a refs database and a key book.
The host builds one per workspace (guides/blocks/host.md); `be` uses the same
crate.

## Connection

`Connection` is a `be_wasm_ops::net::Peer` (`Side::Guest`) that implements
`Caller`, so the generated `be_block_protocol::client::<interface>` functions
take `&connection`. Below it, `transport/` has a `Writer`, a `Reader` and a
`spawn` per platform (tokio and tokio-tungstenite natively, the browser's
`WebSocket` in `web.rs`). Natively an `http(s)://` URL takes the server's HTTP
posts (`transport/native/http.rs`), and both go through `HTTPS_PROXY`;
`be_block_client::authorize` adds a `Be3-Gateway-Authorization` header for an
origin (native only: a browser cannot add one to a websocket).
`be_block_client::call_once` makes one call that needs no sign-in as a one-shot
post (`post_once`: natively the HTTP transport, on the web a `fetch` with
credentials), which is how a browser sign-in is begun and polled; every
connection's first call is `authenticate` (guides/blocks/server.md, "Signing
in").

It opens connection-wide streams on the first `subscribe()` and per-block ones
as `watch` and `join_session` are called, and pumps every stream into one
`broadcast` of `be_block_client::Event`. A receiver that falls behind re-lists
what it wants and rejoins its sessions, as after a reconnect.

**Deciding the server is gone.** A socket can stay open with nothing behind it
(a laptop that slept, a proxy that stopped forwarding) and the transport never
says so, so `Connection` decides: see `CONNECT_TIMEOUT`, `PING_INTERVAL` and
`PONG_WAIT`. `ClientError::is_unreachable` covers a timed-out connect and a
disconnect. The worker alternates reading frames and writing commands and keeps
reading while a write is under way, so neither side's flood starves the other.

Every deadline reads a `be_block_client::Clock` passed to
`Connection::connect`: `Clock::real()` except in tests, which pass
`Clock::manual()` and `advance` it. `Connection::due()` is the clock time the
connection next acts on silence; be-host's `Harness::miss_a_ping` advances to
each `due()` until it gives up. The fault relay (guides/blocks/server.md,
"Tests") supplies the silent server.

## Reading and writing

- `open` reads the head commit and its objects in one `ReadContents`, and
  decodes with `BlockContent::from_stored`; `read_contents` returns `Stored`
  (bytes and the parts they were saved in).
- `save` seals `BlockContent::parts` and publishes against the head it read,
  carrying the commit and the objects (nodes and chunks) that head lacks, so a
  save is one round trip and costs what changed. The outbox keeps a commit's
  full object and reference lists; `send_pending` sends them as changes from
  the expected head's (`held_by`, `references_of`). A merge, or new objects over
  half a frame, upload first (`push`): only what `MissingObjects` says is
  missing, through URLs (below).
- `read_contents` reads many heads at once (one `ReadContents` per 256 blocks,
  chunks that fit in half a frame), so a tree costs one round trip, not three
  per block. Export and `be download` use it.
- `stream_header` and `stream_range` read a `Streamed` type without its
  payload.
- `fetch` verifies hashes; `fetch_history` must run before asking history a
  question.

### Objects

Bulk object bytes never travel inside frames (guides/blocks/server.md,
"Objects over S3"). `fetch` and `try_fetch` ask `download-urls`, at most
`MAX_OBJECT_URLS` at a time, and GET each with `http::get`, at most
`MAX_OBJECT_BYTES` and `TRANSFERS` at once (`peer/transfers.rs`); an object
with no URL, or one the store answers 404 for, is missing, and bytes that do
not hash to what was asked are `ClientError::Tampered`. `push` and imports ask
`upload-urls` with each object's length and PUT the bytes with the headers
given, natively and from the web build alike (`crate::http`: in the browser a
`fetch` without credentials). An import uploads a batch's objects before
sending its creates and publishes, since those are sent as soon as they are
made. A transfer that may succeed later (`http::Error::Failed`: unreachable,
5xx) is `ClientError::Disconnected`, which callers retry or keep for later;
any other refusal is `Refused(Storage, ..)`.

## Live sessions

A block being edited is in a session, which each device holds in its type
module's replica (guides/type_modules.md); the peer only relays, seals and
stores for it. `Peer::relay` puts a relay on the connection without waiting for
the answer to the one before, so a pen stroke's thousands of operations cost no
more round trips than one.

The owner sequences and broadcasts. A follower applies its own edit at once,
rebases it over operations that arrive meanwhile, and reconciles against the
owner's echo; a replica keeps `confirmed` and `visible` (confirmed plus
pending). Only the owner seals, at pauses (guides/type_modules.md, Peers): a
seal writes a commit, heartbeats `clean_at`, rebases the journal, and tells
the followers with `SessionMessage::Sealed` which commit holds which sequence
number.

- **One edit in flight.** A follower's typing waits in the replica's `unsent`
  queue while an edit is unconfirmed, folded by `LiveEdit::absorb_operation`
  (`SeqOp::absorb` for a sequence), stopping before the relay size limit.
- **`Sealed` keeps the follower's head right, and ownership depends on it.** A
  follower that takes over carries on from the last sealed head and the old
  numbering, accepts its pending operations as its own, and owes a seal for
  what the old owner accepted after its last one. With the head wrong, the new
  owner's first save is refused and resume reads its unsaved work as something
  to fast-forward over.
- **Ownership moves under a live owner** when a host's `Event::Claim` takes a
  lapsed lease (a terminal host takes the block from the viewer that opened
  it). The old owner hears it from the next `Lease` and becomes a follower;
  what it accepted meanwhile reaches the new owner as `Accepted` from a
  non-owner, which `Sequencer::inherit` keeps when it continues the numbering,
  and otherwise the new owner applies it and its next seal reloads every
  follower.
- **Catching up.** A follower is catching up until the owner's `Snapshot`
  arrives and resends `Catchup` to each new owner. A replica ignores
  `Event::Claim` while catching up, and one promoted while catching up makes
  its first seal a reload: otherwise the old owner's unsaved operations would
  be lost.
- **Conflicts.** A merge that conflicts first stashes the owner's unsaved side
  as a commit (`Command::Stash`), so the merge has both sides as parents, and
  reports a `be_block_type_host::Conflict`; be-host keeps it per block
  (`be_host_stack::conflicts`) and shows "See both versions".
- **Incompatible messages.** A session message this build cannot decode marks
  the session incompatible; on a follower also diverged, and a diverged session
  refuses to edit or seal.

## The journal

What the owner sequenced since its last commit is kept durable in the block's
journal, so a commit can wait for a pause. A journal is a base (a commit, or
none for no content; the last sequence number it holds; the session state as
of it) and entries (sequence number, lease generation, payload), all sealed
with the block key; the server keeps it beside history, never in it
(`be_server::journal`), and records the module tag that wrote it.

- `Peer::append` seals entries, keeps them in the device's journal in the refs
  database and, online, sends `sessions.append`, which relays each entry to the
  other participants as a relay would and keeps it when the caller owns the
  session's current generation: one call broadcasts and appends. Entries at or
  below the last sequence number are dropped as already held.
- `Peer::rebase_journal` starts the journal over from a base, dropping every
  entry, and keeps the entries it is given past the base in the same write on
  the device (online, the server gets a rebase and then an append): an owner
  rebases after each commit, and on taking a session before it appends.
- **A seal on its way.** The owner keeps journaling what is typed while its
  seal waits on the server. The seal's commit records the session generation
  and last sequence it holds (`Commit::sealed_through`); once it lands the owner
  rebases onto it carrying what came after, and followers, who applied that
  already, reload the seal and hear it again (`SessionMessage::Sealed` with
  `reload`). A device that stopped after the server took the seal and before it
  heard so opens on the commit the seal was made on, replays its own journal
  there and takes the seal as its head, when its journal noted that seal
  (written with the seal's commit in one refs write, dropped by the next rebase:
  `Peer::sealing`) and holds the seal's last entry in its generation
  (`reaches_seal`; guides/type_modules.md, Peers).
- Joining (`Peer::join_session`) answers the journal as of joining, so a
  replica opening on its base needs no state from the owner
  (`be_block_type_host::Replicas` picks the server's or the device's,
  whichever holds more). `read_contents` answers each block's journal
  (`BlockRead::journal`, opened; offline the device's); a reader applies it
  with `Modules::readable`, which replays it through the type when its tag is
  the module's and its base is the head, and reads the head alone otherwise.
  Export, `be download` and the tracker's reads go through it; `be sync` reads
  heads alone, since it records which head it synced and what it writes to
  disk must be that head.
- A journal is its module tag's: an append of another tag is refused until the
  owner rebases, and a replica of another build opens on the head alone and
  reports the edits lost (guides/blocks/host.md, Undo). A rebase of another tag
  that drops entries marks the journal `lost` until it moves to another commit,
  so every device opening the block meanwhile is told too.

## The refs database

A `Peer` keeps what it learns in `RefsDb` (`be_block_client::refs`, SQLite on
every platform, in the origin private file system on the web), keyed by server,
account and organization: local and last-known remote heads, the newest head
whose content it has read whole, raw `BlockSummary`s (metadata stays sealed),
key rows and wraps, verified contacts and escrow chain, the workspace list, the
outbox, the unsent area and each block's journal. A sealed commit and its local
head are written in one transaction (`Refs::push_publish`), and so is a publish
settling: its outbox entries leave as the remote and local heads move on
(`Refs::settle`), so no reader sees an empty outbox beside the old heads. The
remote head never moves back to one it descends from (`Peer::behind`): the
server's head only moves on, so such a head was heard of late, from a summary
read before this device published and answered after, or from a merge settling
a divergence a later one already settled. A server restored from a backup, whose
head is older, is caught up by the next publish: rejected with the server's
real head, the device merges against it (`Peer::reconcile`) and the published
merge moves the remote head on. A record that no
longer decodes is skipped on load.

It keeps one summary per block, the newest revision winning, and a listing is
the ids it held: a copy kept per listing went stale offline (a listing fetched
before another device's seal, or the copy a parent's listing held). Offline, a
listing answers as the summaries place its blocks now:

- a block's `References` are what its summary references, so a seal or an
  offline commit after the listing was fetched shows;
- its `Backrefs` are the summaries that reference it;
- a block's `Children`, and the `Deleted` blocks, are those the listing held
  that their summaries still put there, then those whose summaries put them
  there and are newer than the revision the server answered the listing at
  (`Refs::listed`, kept with the listing). So a block made online, moved in,
  or pushed since the listing was fetched shows, and one moved away does not.

A summary is dropped only with the blocks `forget` drops, so one the server no
longer lists stays until then.

`Peer::cached` builds a peer from it with no network, and `Peer::attach` hands
it a signed-in connection later (after every reconnect), which opens the
organization and refreshes keys and contacts. While disconnected, `summary`,
listings and `read_contents` answer from the cache, and `open` reads the local
head first while it has unpublished commits.

A summary's head is the server's newest, which this device may never have read
(another device moved it). Offline, content is read at `Peer::held_head`: the
first whose objects the store holds whole of the local head, the server's, and
the head last read whole (`Peer::read_whole`, which `read_contents` and a
replica's whole load record). A block with none has nothing to read offline.

A push notification (guides/blocks/server.md, "Notifications") opens with
`be_lib_web_push::open` (the device's own RFC 8291 keys; a browser does this
itself) to a `be_block_protocol::PushMessage`: organization, block, the sender
the server vouches for, then the text sealed under the block's data key with
the organization, block, sender and send time. `Peer::open_notification` opens
that offline from the cached key rows, with no block load and no type module,
refusing one sealed about another organization, block or sender, or older than
`MAX_NOTIFICATION_AGE_MILLISECONDS`.

## Saving and the outbox

A save is a `be_block_client::Write` in two steps: `Peer::seal` writes the
commit, records it in the outbox and moves the local head; `publish_sealed`
sends it. `Peer::write` does both.

- **Offline**, a failed publish stays in the outbox, as do creates
  (`refs::Op::Create`), moves (`Op::Move`) and defines on a block with no
  content (`Op::Define`); these rewrite the cached summary so listings show
  them, and later ones for a block queue behind earlier ones. `keep_creation`,
  `keep_definition` and `keep_move` put them there online too (the host's
  path); a publish waits behind a creation, move or define of its block. Every
  summary the server gives is laid over with what the outbox has yet to send of
  its block, a waiting move's parent and a waiting define's metadata, as it is
  kept (`RefsDb::set_listing`, which `Peer::note` and listings keep it
  through) and where it is returned (`Peer::summary_with_outbox`), and a server
  listing with what the outbox adds or moves (`Peer::with_outbox`): an answer
  the server gave before those reached it would otherwise undo them here.
  Sending a move carries its databases' prefixes along (`Moving::from`). Sharing, access,
  members, invites, agents, contacts and pairing need the server.
- **`Peer::drain`** publishes the outbox in order once attached and reports a
  `Drained` per block. A rejected publish whose remote head already contains
  the commit is settled, not diverged; an entry is settled once a published
  head descends from it. `Peer::reconcile` finishes a diverged block, answering
  `Reconciled::Merge` when the replica host has to merge.
- **The unsent area.** A block whose access is refused (on a publish, a create,
  `summary`, or a drain refused `PermissionDenied`/`BlockNotFound` after
  re-reading the summary) moves its entries to `Peer::unsent`;
  `restore_unsent` brings them back, `discard_unsent` deletes them, and
  `UnsentBlock::offerable` says the summary allows editing again.
  A create refused for any other reason (over a quota, too large) is set aside
  the same way, so nothing waits behind it, with the code it was refused with
  (`UnsentBlock::refused`, `Drained::SetAside`): the host shows it, an
  organization's lost-access policy never deletes it, and cached listings leave
  it out. What is made in it is set aside with its refusal, and restoring it
  restores them. A publish refused as a change the server will never take
  (`InvalidRequest`, `ObjectTooLarge`) is set aside the same way rather than
  sent again every round. A refused move or define puts back the summary the server has
  (`unchanged`, `Drained::Refused` with `restored`).
- `Peer::pending` counts outbox operations, local commits, their blocks and
  unsent blocks.

## Tests

The round trip through a real server is tested in
`crates/be-block-client/src/tests/`, with `Harness` in `src/tests.rs`.
