# The server and the storage crates

Part of the block stack (guides/the_new_block_stack.md). Keys and contacts on
the server are in guides/blocks/keys.md.

## Storage crates

- **be-block-store**: content-addressed, encrypted objects. `Hash` is the
  SHA-256 of the *ciphertext*, so the server verifies what it holds without
  reading it. `chunker::split` is content-defined, so an edit in the middle of a
  large file re-uploads one chunk. A `Manifest` (`{content_type, length,
  chunks}`) plus `Vault::read_range` is how a large block streams. `ObjectStore`
  is synchronous; fetching over the network is explicit (`Peer::fetch`).
  Durability rules are on `ObjectStore::sync`.
- **be-block-commit**: a `Commit` (`{parents, manifest, author, time, kind,
  references, metadata}`) is an encrypted object; `Publish` also sends its
  parents in plaintext, so the server knows every block's history graph.
  `references` are derived from the *sealed* state, so the reference graph
  always describes content that was stored. `merge` is diff3
  (`merge_slices`, with `merge_lines` and `render_conflicts` on top).
  Retention (`retention::plan`) runs only when a client calls `Peer::prune`,
  and nothing calls it on a schedule yet.
- **be-block-session**: `Lease` (ownership), `resume` (whether a returning peer
  fast-forwards, publishes or merges, decided from history rather than from
  having been away), and `Sequencer`/`Follower`. Operations carry an `OpId`, so
  work resubmitted after a reconnect is deduplicated. Lease length is
  `ServerConfig::lease`; tests that claim pass zero (be-host's
  `Harness::lapsing`, `Lease::lasting(0)` in the replica's `Sim`).
- **be-block-graph**: liveness and access.

### Liveness and reclamation

A block is live when its parent chain reaches `BlockParent::Root` and every
link holds: a parent holds a child only while the parent's references list it
(`BlockGraph::holds`). So deleting a block is an edit to its parent's content
that drops it (`ChildChange::Delete`), and undo or Restore lists it again. A
block whose parent is `Detached` or no longer lists it is in the trash with its
subtree, and keeps its parent. Any other reference is a link: navigation and
access, never liveness. Each block has one parent, so liveness is one walk up
and reclamation is reference counting (`ObjectRefs`).

Every content type therefore lists its children among its references, and
whatever makes a child under a parent also adds it to the parent's content.
Until that edit is saved the child is in the trash for the parent's other
viewers; the host hides that window on its own side (`Graph::placing`).

`CollectDetached` (`Peer::collect_detached`) removes every deleted block the
caller may edit with its objects, but nothing calls it yet. Once something
does, content must cope with a linked block that is gone, and collection will
race a new child its parent has not listed yet.

## be-server

SQLite for metadata (`schema.rs`, checked on open by
`be_block_sqlite_schema::ensure`, which only lets a schema grow; the host's
`host.sqlite3` is checked the same way and recreated, signing the device out,
when it does not match), a `FileStore` for objects, and postcard frames over a
websocket. Where a websocket cannot get through, `POST .../http` carries the
same frames (`http.rs`): the greeting's answer names a session in a
`Be3-Session` header, and each later post carries queued client frames and is
answered with whatever the server has for the client; notifications wait for
the next post. An answer stops taking frames once its replies fill
`OUTBOUND_BYTES`, keeps the rest for the next post, and says so with
`Be3-More`, on which the client posts again at once (with an empty body if it
has nothing to send). The client gives up on a post only after `QUIET_WAIT`
with no bytes moving either way, so a large post on a slow link goes through.

### Protocol

- **Greeting.** Each side sends `be_block_protocol::hello` with its build id
  (`be_lib_build_id::current()`, from `build-info.json` beside the executable
  or `--build-info-file`), and the server closes the connection when they
  differ (`ClientError::Incompatible`). A program with no id fails
  (`ClientError::NoBuild`; the server does not start); tests install one with
  `be_lib_build_id::install`. Server and clients ship from one commit, so the
  protocol is never versioned (`decisions/server_protocol_is_unversioned.md`).
- **Calls and streams.** After the greeting every frame is a be-wasm-ops frame
  (guides/type_modules.md, "Operations and WIT") of the WIT package
  `be3:server` (`crates/be-block-protocol/wit`, generated with `network: true`):
  each call is an op, with no request ids. A function answers `result<T,
  error>`; client code is `client::blocks::read_block(&connection,
  block).await??` or `ask(...).await?`. What the server pushes is a stream:
  `blocks.changes`, `blocks.watch(block)`, latest-value streams
  (`account.paired`, `contacts.contacts-changed`, `keys.key-work-changed`), and
  a `sessions.session` resource whose `join` stream carries the session's
  changes and relays; dropping it leaves the session.
- **Who may call what.** Every call states its requirement once, in `rule`
  (`client.rs`): anyone, signed in, signed in with an organization open, an
  administrator (of the open organization, or of the one named), or at least
  some access to the blocks it names (a session's calls: to its block). The
  dispatcher (`Client::admit`) applies it before the handler runs, which then
  checks only what a rule cannot say, under the graph lock. Only `ping` and the
  sign-in calls (`begin-browser-login`, `finish-browser-login`,
  `authenticate`, `create-local-profile`) need no sign-in, and every stream
  needs it, so a client opens its streams once signed in.
  `every_call_states_who_may_make_it` makes every call of `be3:server`
  (`embed::METHODS`) and fails until a new one is made there too. A block the
  caller may not know exists is refused as `BlockNotFound`, like one that does
  not exist; `PermissionDenied` means it may know it exists.
- **The server's side** is a `be_wasm_ops::net::Peer` (`Side::Host`,
  `client.rs`). It answers calls in arrival order, except a call that waits
  (`finish-browser-login`), which a task answers and hands back to the loop
  (`Late`). `WatchHub` sends each connection `Push`es, written to its open
  streams (one `be_wasm_ops::Outlets` keyed by `Route`). Slow consumers are
  slowed or disconnected: `STREAM_BUFFER`, `OUTBOUND_FRAMES`. A ping is
  answered by the socket's reader task (`serve_socket`) ahead of queued calls,
  so a slow call never makes a client think the server is gone.
- **Limits.** Nothing a client sends makes the server hold memory it does not
  bound, and a client that does not read is disconnected rather than buffered
  (`lib.rs`, `http.rs`):
  - A stream's window never grows past its first size (`be_wasm_ops::Window`),
    and a connection's outlets hold back at most `STREAM_BUFFER_BYTES`.
  - The loop takes no frame while `OUTBOUND_FRAMES` or `OUTBOUND_BYTES` wait to
    be written; past `MAX_OUTBOUND_BYTES` waiting (`Outbound`), the connection
    or HTTP session is closed.
  - `MAX_CONNECTIONS` overall and `MAX_CONNECTIONS_PER_ADDRESS` for one
    address; `MAX_HTTP_SESSIONS` and `MAX_HTTP_SESSIONS_PER_ADDRESS` likewise.
    HTTP sessions are swept every `SWEEP_INTERVAL` by a task off the accept
    loop, which also closes sessions whose token was revoked.
  - A connection without an account is closed `sign_in_timeout`
    (`ServerConfig`) after its greeting or logout, unless it waits on a browser
    sign-in, until that expires. Clients connect only to authenticate or to
    begin a browser sign-in at once (`be_host_stack::link`, `accounts.rs`), so
    the deadline never closes a connection be-host keeps. Pending browser sign-ins and portal form
    tokens are bounded per address (`PENDING_PER_ADDRESS`,
    `FORMS_PER_ADDRESS`; another lets go of the address's oldest), and one
    call waits on a code at a time.
  - Request bodies grow only as their bytes arrive and stall out after
    `BODY_READ_TIMEOUT`. A websocket is idle only after `IDLE_TIMEOUT` with no
    bytes read at all (`next_heard`), so a large message on a slow link is
    not cut off however long it takes.

  An address is a client's `Services::client_address`: its peer, or the
  `X-Forwarded-For` hop nearest it when the peer is in `trusted_proxies`
  (loopback by default, `--trusted-proxy` for more). Per-address limits count
  an IPv6 /64 as one address (`address::address_key`).
- **No batch request.** Calls on one connection are answered in order and both
  transports send whatever is queued together, so independent requests cost one
  round trip; only a chain where each needs the last one's answer costs more.
- **Publishing** is a compare-and-swap on the head: a stale publish comes back
  `Rejected` with the head to merge against. It writes the head, the edges the
  commit declares and refcounts for its new chunks.
- **Storage** is charged to each block's author (an agent's creator), counting
  each distinct object once per account and every version a block keeps
  (`decisions/storage_is_charged_to_the_author.md`; `store/usage.rs`). An
  account may store `ServerConfig::account_quota` bytes, and an organization
  its settings' `storage_quota` bytes; an upload, publish or hold that would
  pass either is refused with `OverQuota` before anything is written, and
  nothing that frees space is ever refused. `PutObjects` charges objects no
  block holds to the uploader (`object_uploads`, `pending`) until a block takes
  them; `collect_uploads` deletes them after `upload_grace`, when the server
  starts and every `UPLOAD_SWEEP_INTERVAL`. Counters are kept incrementally:
  `object_sizes`, `account_objects` (per payer and object, how many of its
  blocks hold it), `account_usage`, `organization_usage`, and `block_sizes`,
  which the cached `BlockGraph` mirrors as `BlockNode::size` and `tree_size`
  (`add_size`, `set_parent` and `remove` carry a subtree's bytes up its
  ancestors). `BlockSummary` reports both; `account.storage-usage` reports the
  account's `held`, `pending` and `quota`, and `account.storage-changed` streams
  it: its value on opening, then again whenever the payer's `account_usage` row
  changes (`ServerStore::usage_changes`, a broadcast of payers each connection
  following its own filters; one that falls `USAGE_CHANGES_QUEUED` behind reads
  again). The first time a server opens data
  kept before these tables, `usage::backfill` fills them once
  (`server_backfills`), and counts objects on disk that nothing holds as
  uploads by nobody.
- **Objects** are served only to a member who can view a block that holds them
  (by a commit, `commit_objects`, or by `HoldObjects`); anything else reads as
  missing. A merge commit holds its extra parents' objects, so a stash that was
  never a head stays readable. Each upload is recorded per organization and
  account (`object_uploads`: size and first upload time), and a block may hold
  or publish an object only if its caller uploaded it there or may already read
  it (`holdable` in `blocks.rs`); `MissingObjects` answers by the same rule, so
  knowing a hash neither reads it nor tells whether someone else stored it.
- **Watches and live sessions** are keyed by organization and block
  (`watch::Place`): a block id is the client's choice, so another organization
  can hold a block with the same one. Watching or joining needs `View`, and
  taking a session's lease needs `Edit` (a viewer's claim is refused, and the
  replica host ignores the refusal).

### Organizations and access

The tenant is an organization (everything keyed by `organization_id`): one
block graph, its members and administrators. Its `kind` is `company` (made by
`CreateOrganization`) or `personal`, the server's one shared organization
(`ServerStore::personal_organization`), which registering joins as an editor
and nobody administers. An organization always keeps one administrator;
changing a member closes their connections (`WatchHub::drop_member`) so they
reconnect with what they now are.

Access is per member (`Visibility` in `blocks.rs`, `be_block_graph::Access`:
`None < KnowExists < View < Edit < Manage`): administrators manage everything;
anyone else gets the best grant on the block or an ancestor (a lower grant
never reduces an inherited one), and `Manage` on what they authored while they
can view its parent (or it is `Root` or `Detached`), so a member taken off a
workspace loses what they wrote inside it. Settings blocks and edit kinds take
their access from their parent: guides/settings.md, "Access".

- A block someone can view that references another gives them `KnowExists` on
  it, one level only: they learn its id, content type and access
  (`BlockSummary::no_access`) and are served none of its keys or objects, and
  may not watch it or join its session. Anyone can give themselves
  `KnowExists` on any id by listing it in their own block's references, so it
  never grants more than that summary.
  A viewable block whose parent is not viewable is an entry point (Shared with
  me).
- Moving (`SetParent`) needs `Edit` on the block and the new parent, and
  `Manage` to move a workspace or into another workspace, or when the move
  would raise the mover's own access (`BlockGraph::move_refusal`).
- `SetAccess` needs `Manage`, and a grant above `None` must name a member who
  is a contact of the granter (guides/blocks/keys.md). Anyone may revoke their
  own grant. `ListAccess` never lists the rest of the membership.

No check maps the whole graph. `BlockGraph` indexes children by parent and
blocks by the accounts that authored or were granted them; `inherited` walks
up from one block (every edit and view check), `visible` walks down from an
account's entry points, and `audience` names who may know a block. A move or
grant changes what is below only for accounts in its audience before or after,
so nothing walks the subtree. The graph is cached per organization
(`ServerStore::with_graph`) and dropped when an action fails in a way that may
have left it apart from the database: any error but a refusal, and a refusal
after the graph (`BlockGraph::changes`) or the database (`total_changes()`)
changed. A refused call therefore costs no reload. A member's `Deleted` listing
and `CollectDetached` walk what the member can see (`Visibility::candidates`),
not the organization, since the personal organization holds every account's
blocks.

### Announcements and listings

A connection hears only of what it listed. `list-blocks` (a `Listing`:
children, `Shared`, backrefs, references, `Deleted`, `OpenSettings`) and
`read-block` register with `WatchHub` *before* reading, so nothing changed
during the read is missed, and the hub remembers each block it told a
connection of. A change is sent to each connected member in the block's
audience before or after (and every administrator), as the `BlockChanged` that
member's access allows or a `BlockRemoved`, when a listing matches the block
now or the connection heard of it before (so a block moving out of a listed
folder is still announced, then forgotten). The blocks a change starts or stops
referencing are announced with it. `unlist` drops a listing; a refused listing
stays until then, so a block read before it is made is announced once it is,
but a connection keeps at most `REFUSED_LISTINGS` of them (`watch.rs`) and
drops any refused past that at once.

Every `BlockSummary` and `BlockRemoved` carries a `revision`
(`ServerStore::with_graph`). Replies and announcements can arrive out of order
(a reply overtakes announcements queued before it), and the revision is how a
client puts them back in order.

### Limits

`limits.rs` names what one account or connection may make the server keep:
blocks authored per organization (counted in the graph, `BlockGraph::authored`),
organizations created, open contact links, contacts, invitations outstanding and
escrow keys per organization (refused as `OverQuota`); and per connection,
sessions, listings and streams. Names, metadata (held in the graph and sent in
every summary), sealed blobs, pairing messages and relayed session messages
have byte limits (refused as `InvalidRequest`). A relay is shared by every
participant's queue (`Push::Relayed` holds an `Arc`).

The session registry (`sessions.rs`) indexes leases by client, so leaving them
all on a disconnect costs what that client joined, and remembers an empty
session for its `clean_at` for a day and at most `MAX_EMPTY_SESSIONS`; a peer
taking over a forgotten one merges rather than fast-forwards.

Nothing the store does blocks the runtime's threads. Its SQL runs as owned
jobs on tokio's blocking pool, one at a time under the database lock:
`ServerStore::with_database(move |store, database| ...)`, and `with_graph`,
which adds the organization's graph. A job captures what it needs by value
(the store is a cheap clone) and runs to its end even when its caller is
dropped. Uploads are written and synced, and objects read, on the blocking
pool too, outside the lock (`put_objects` after it reserves the quota,
`get_object`, `get_objects`). In tests, `store::held::hold(organization)`
keeps an organization's next graph job waiting until released. A connection
resolves its token again only after `ServerStore::sessions_changed`, not on
every call and push (`Connection::signed_in`), so every path that deletes or
changes an app session calls it.

### Accounts

Remote accounts register and sign in through the `/auth` portal, which
verifies the email before creating the account; the protocol has browser
sign-in handoff (`finish-browser-login`, called again after a reconnect),
token `Authenticate`, header authentication and `Logout`. Registering needs an
invite code the operator made (`store/invite_codes.rs`), checked with every
other cheap check before the password is hashed and spent when the verified
account is made. Password hashing and checks (`store/passwords.rs`) run off the
database lock, a few at a time, behind a bounded queue with a small share per
address (an IPv6 address counts by its /64); past it the portal answers busy.
Failed checks lock out an (email, address) pair after five, an address after
more, and an email from every address only after many more, doubling up to an
hour and forgetting one failure per quiet `FAILURE_DECAY`; the records are
bounded. Mail links are capped per email and purpose, per address and overall
(`store/portal.rs`); a reset for an email with no account does the same work
and sends nothing. `Be3-Gateway-Authorization` is left to a gateway in front.

An administrator `Invite`s an email under an id the inviting client picks;
only an account with that email can read or accept it, once, and the id
reaches the invitee only in the link (guides/blocks/keys.md, "Identity, new phrases and devices").
`CreateAgent` makes a passwordless editor account with a token
(guides/tracker.md), at most `MAX_AGENTS_PER_CREATOR` per creator; only its
creator lists or removes it. Accepting an
invitation, or creating an agent, makes the two accounts contacts.

## Running it

```
./scripts/buck run //crates/be-server-bin:bin -- --addr 127.0.0.1:9090 --data-dir be-server-data --allow-login --allow-registration
```

Make an invite code with `be-server invite-code create --data-dir
be-server-data` (through the same `:bin -- `), then register at
`http://127.0.0.1:9090/auth` with it; until email delivery is connected,
verification messages are printed in the server log. Deployments also set
`--auth-origin` (guides/hosting.md). The native host embeds be-server for a
local account (`be-host-stack/src/embedded_server.rs`, ephemeral port, data
under the host's); `./scripts/buck run //crates/be-host:web` starts one beside
the web build.

## Tests

- `be_server::Running` serves a fresh temporary directory on an ephemeral port
  in the current runtime and removes it when dropped: `Harness` in
  `crates/be-server/src/tests.rs` and `crates/be-block-client/src/tests.rs`.
- be-host-stack's `Harness` (`src/tests.rs`) starts the embedded server and the
  host's peer against it; crates above it use `be-host-testing`'s `Harness`,
  which serves the first-party types in process
  (`be_host_stack::testing::serve_in_process`).
- `crates/be-block-fault-relay` sits between a client and a server and can
  `stall`, `stall_at_next_request`, `cut`, `hold_new` or `jam_at_next_request`
  connections, and `count` the calls the client starts (method ids are
  `be_block_protocol::__args::<INTERFACE>_<FUNCTION>`), so a test waits for a
  ping or heartbeat to be sent.
- The graph's tests check access against random graphs
  (`be-block-graph/src/tests.rs`).
