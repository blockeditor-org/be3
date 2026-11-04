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

- `open` reads the head commit and its chunks in one `ReadContents`.
- `save` publishes against the head it read, carrying the commit and the chunks
  that head lacks, so a save is one round trip. A merge, or new objects over
  half a frame, upload first: only what `MissingObjects` says is missing, in
  `PutObjects` of up to 2 MiB.
- `read_contents` reads many heads at once (one `ReadContents` per 256 blocks,
  chunks that fit in half a frame), so a tree costs one round trip, not three
  per block. Export and `be download` use it.
- `stream_header` and `stream_range` read a `Streamed` type without its
  payload.
- `fetch` verifies hashes; `fetch_history` must run before asking history a
  question.

## Live sessions

A block being edited is in a session, which each device holds in its type
module's replica (guides/type_modules.md); the peer only relays, seals and
stores for it. `Peer::relay` puts a relay on the connection without waiting for
the answer to the one before, so a pen stroke's thousands of operations cost no
more round trips than one.

The owner sequences and broadcasts. A follower applies its own edit at once,
rebases it over operations that arrive meanwhile, and reconciles against the
owner's echo; a replica keeps `confirmed` and `visible` (confirmed plus
pending). Only the owner seals: a seal writes a commit, heartbeats `clean_at`,
and tells the followers with `SessionMessage::Sealed` which commit holds which
sequence number.

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

## The refs database

A `Peer` keeps what it learns in `RefsDb` (`be_block_client::refs`, SQLite on
every platform, in the origin private file system on the web), keyed by server,
account and organization: local and last-known remote heads, raw
`BlockSummary`s (metadata stays sealed), key rows and wraps, verified contacts
and escrow chain, the workspace list, the outbox and the unsent area. A sealed
commit and its local head are written in one transaction
(`Refs::push_publish`); a record that no longer decodes is skipped on load.

`Peer::cached` builds a peer from it with no network, and `Peer::attach` hands
it a signed-in connection later (after every reconnect), which opens the
organization and refreshes keys and contacts. While disconnected, `summary`,
listings and `read_contents` answer from the cache, and `open` reads the local
head first while it has unpublished commits.

## Saving and the outbox

A save is a `be_block_client::Write` in two steps: `Peer::seal` writes the
commit, records it in the outbox and moves the local head; `publish_sealed`
sends it. `Peer::write` does both.

- **Offline**, a failed publish stays in the outbox, as do creates
  (`refs::Op::Create`), moves (`Op::Move`) and defines on a block with no
  content (`Op::Define`); these rewrite the cached summary so listings show
  them, and later ones for a block queue behind earlier ones. Sharing, access,
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
- `Peer::pending` counts outbox operations, local commits, their blocks and
  unsent blocks.

## Tests

The round trip through a real server is tested in
`crates/be-block-client/src/tests/`, with `Harness` in `src/tests.rs`.
