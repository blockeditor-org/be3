# The server and the storage crates

Part of the block stack (guides/the_new_block_stack.md). Keys and contacts on
the server are in guides/blocks/keys.md.

## Storage crates

- **be-block-store**: content-addressed, encrypted objects. `Hash` is the
  SHA-256 of the *ciphertext*, so the server verifies what it holds without
  reading it. `chunker::split` is content-defined, so an edit in the middle of a
  large file re-uploads one chunk. A `Manifest` (`{content_type, root}`) names
  the root of a tree of sealed `Node`s over the chunks, in the shape of the
  content's parts (guides/blocks/content_types.md, "Parts"); a leaf split over
  many chunks is grouped under nodes chosen by the chunks' hashes. Unchanged
  subtrees are shared between commits while the data key stays the same: a
  write reseals a reused part sealed under another key (`Vault::stale_parts`
  names them, and `Peer::write_content` fetches their objects first), so after
  a rotation the next commit, a rekey's seal among them, holds nothing the old
  key alone opens, and a subtree is always sealed under one key (its top
  object's, `Vault::sealed_under`). `Vault::objects` lists
  a tree's nodes and chunks;
  `missing_objects` names what is absent a level at a time, which
  `Peer::fetch_range` fetches until none is left. `Vault::read_range` is how a
  large block streams. `ObjectStore`
  is synchronous; fetching over the network is explicit (`Peer::fetch`).
  Durability rules are on `ObjectStore::sync`.
- **be-block-commit**: a `Commit` (`{parents, manifest, author, time, kind,
  references, metadata}`) is an encrypted object; `Publish` also sends its
  parents in plaintext, so the server knows every block's history graph.
  `references` are derived from the *sealed* state, so the reference graph
  always describes content that was stored. A commit's `references` are the
  content's own and its metadata's; the parts of its manifest tree hold the
  rest in their nodes (`Node::references`), so an unchanged part shares them
  between commits. `Peer::references_of` lists them all. `merge` is diff3
  (`merge_slices`, with `merge_lines` and `render_conflicts` on top).
  Retention (`retention::plan`, `RetentionPolicy::default()`: one state per
  minute for the last hour, per 10 minutes for a day, per hour for a week, per
  day for 30 days, per week after) runs on the server as each publish arrives,
  in its transaction: the new head wins its slot, so the commit it replaces
  there is dropped with whatever has aged into a wider slot. The head,
  bookmarks, merges' parents and everything inside the organization's
  `history_kept_for` (judged by when the server received it too) stay. A
  content type whose old states are worth little has a shorter policy built
  into the server (`be_server::retention`, by the content types `be-block-graph`
  names: the search index keeps a few states of its last day, dropping older
  ones with `drop_after`), never keeping less than `history_kept_for`. A
  pruned commit truncates a chain; a device merges an offline edit against
  its last-known remote head, which it holds locally, so the server pruning
  that head costs nothing (`Peer::reconcile`). A reader that has a head but
  not yet its objects can find them gone once a newer head replaces it in its
  slot: it reads the server's head again (`be_host_stack::search::write`). The
  indexer gives a batch back after a few such heads in a row and holds it until
  the index's head moves on, which the mirror lists the index for, or until a delay
  that doubles with each batch given back runs out (`Searching::moving`).
- **be-block-session**: `Sequencer` (the order's rules: each submission takes
  the next number and the floor, the last barrier, which a barrier moves to
  itself), `Membership` (who is in a session, over the `session-state` the
  server and every replica hold: `write` and `apply` its entries, `holder`,
  and the build the session speaks, its first participant's, `speaks`),
  `resume` (whether a returning peer
  fast-forwards, publishes or merges, decided from history rather than from
  having been away) and the `SessionMessage`s replicas relay. A submission's
  `OpId` is its replica's `origin`, random and kept across its reopenings, and a
  counter it never repeats, so the server answers an id its journal holds
  without numbering it again.
- **be-block-graph**: liveness and access.

### Liveness and reclamation

A block is live when its parent chain reaches `BlockParent::Root` and every
link holds: a parent holds a child only while the parent's references list it
(`BlockGraph::holds`). The one exception is an account's Unfiled
(`BlockNode::unfiled`, a block of `UNFILED_TYPE`, which only the account's
reserved id may be), which holds every child whose parent names it. So
deleting a block is an edit to its parent's content that drops it
(`ChildChange::Delete`), and undo or Restore lists it again. A
block whose parent is `Detached` or no longer lists it is in the trash with its
subtree, and keeps its parent. Any other reference is a link: navigation and
access, never liveness. Each block has one parent, so liveness is one walk up
and reclamation is reference counting (`ObjectRefs`).

Every content type therefore lists its children among its references, and
whatever makes a child under a parent also adds it to the parent's content.
Until that edit is saved the child is in the trash for the parent's other
viewers; the host hides that window on its own side (`Graph::placing`).

Nothing deletes a block for good: a block in the trash stays with its history
(decisions/garbage_collection_waits_for_trash_and_history.md). Collection, if
it comes, is a server policy; then content must cope with a linked block that
is gone, and collection will race a new child its parent has not listed yet.

## be-server

SQLite for metadata (`schema.rs`, checked on open by
`be_block_sqlite_schema::ensure`, which only lets a schema grow; the host's
`host.sqlite3` is checked the same way and recreated, signing the device out,
when it does not match), an object store (`objects.rs`, "Where objects live"
below), and postcard frames over a websocket. Every transaction that writes
begins with `be_block_sqlite_schema::write_transaction`, which takes the write
lock up front, so a second connection to the database (a test or tool opening a
`ServerStore` beside a running server) waits out `BUSY_TIMEOUT` rather than
failing with `SQLITE_BUSY`.

be-server parses no HTTP itself (decisions/be_server_serves_http_with_axum.md): the accept loop
(`serve_with_mail`) admits each connection by its peer's address and hands it to `http::serve`,
which serves it with hyper's HTTP/1 connection. Its `front` counts each request against its
client's places (below, "Limits") and passes it to the route its path ends with, each an axum
`Router` with its own body limit (`http::Routes`): `/auth…` the portal (`http/auth.rs`),
`…/storage/objects…` the S3 endpoint, `POST …/call` a one-shot call, `POST …/http` a post of the
long-poll fallback, and anything else a websocket upgrade, which hyper hands over once answered
and tokio-tungstenite serves (`serve_socket`). Paths are matched by their end, since a proxy may
mount the server below any path.

Where a websocket cannot get through, `POST .../http` carries the
same frames (`http.rs`): a post with no session carries the greeting, and
its answer, once the greeting signs in, names a session in a
`Be3-Session` header, and each later post carries queued client frames and is
answered with whatever the server has for the client; notifications wait for
the next post. An answer stops taking frames once its replies fill
`OUTBOUND_BYTES`, keeps the rest for the next post, and says so with
`Be3-More`, on which the client posts again at once (with an empty body if it
has nothing to send). The client gives up on a post only after `QUIET_WAIT`
with no bytes moving either way, so a large post on a slow link goes through.

### Objects over S3

Clients move object bytes only through presigned S3 URLs (decisions/objects_live_in_s3_compatible_storage.md):
`reserve-uploads` takes (hash, length) pairs and answers, per object, none
for one the caller holds already (`held_objects`: it uploaded it, or may read
it) and otherwise a presigned PUT (`upload-url`: the URL and the headers to
send with it), reserving only those as an upload (below, "Storage"). An object
with no length (one the caller reuses by its ref and does not hold) asks only
whether the caller holds it: it is answered none if so, and otherwise refuses the
call as `object-not-found`, alike whether another organization stored it or no
one did, with nothing reserved, so a real uploader is never held to a length no
one gave and the answer leaks nothing. `query.download-urls` answers a
presigned GET for each object the caller may read (`may_read_objects`) and none
for the rest. Each names at most `be_block_protocol::MAX_OBJECT_URLS` objects.
Only `publish`'s carried objects and a `blocks` read's objects travel inside
frames. The URLs are signed by `Services::transfers`, an `objects::Bucket`:
the configured bucket, so bytes go between clients and the bucket and never
through the server, or with none the server's own endpoint below
`ServerConfig::public_url` (`--public-url`; by default `http://` and the
address it listens on), `S3Endpoint::transfers`. A URL lasts
`ServerStore::transfer_lifetime`, at most 15 minutes and under half of
`upload_grace`.

With no bucket, be-server answers S3 requests itself at `.../storage/objects/HASH` (`s3.rs`, served
by `http/s3.rs`, s3s's own tower service behind the CORS headers browsers need; with a bucket it serves none): path-style, one bucket `objects`,
keys the object's hash in lowercase hex, whatever path the proxy puts in
front (`OneBucket` gives s3s the bucket, so signatures cover the full path;
s3s still checks the first path segment is a valid bucket name, so the mount,
`s3::PATH`, is `storage/` and not `s3/`).
Only GetObject (with Range), HeadObject and PutObject are served; PutObject
requires `x-amz-checksum-sha256`, stores only a body whose hash is both the key
and the checksum, and answers once the object is synced. Every request is
SigV4-signed, in practice presigned, with one credential pair derived from
the server's own secret (HKDF-SHA256, `ServerKey::derived`): `ServerStore::s3_endpoint` gives an `S3Endpoint`,
whose `bucket(server_url)` and `credentials()` feed `s3::presign_get` and
`s3::presign_put` (which signs the length and checksum, so the URL takes only
that object; `rusty-s3` signs). Bodies are read whole within
`BODY_READ_TIMEOUT`, up to `MAX_OBJECT_BYTES`. Answers carry
`Access-Control-Allow-Origin: *` and `Cross-Origin-Resource-Policy:
cross-origin`, and preflights are answered, so a cross-origin isolated page
can use the URLs; the signature is the only credential. The endpoint itself
counts and records nothing: `reserve-uploads` did before it signed.

### Protocol

- **Greeting.** Each side sends `be_block_protocol::hello` with its build id
  (`be_lib_build_id::current()`, from `build-info.json` beside the executable
  or `--build-info-file`), and the server closes the connection when they
  differ (`ClientError::Incompatible`). Past a zero byte after the build, a
  client's greeting says its token (`Greeting`) and the server's answers who
  that signed in (`Welcome`); only the build is read before both ends know
  they are of one build. A program with no id fails
  (`ClientError::NoBuild`; the server does not start); tests install one with
  `be_lib_build_id::install`. Server and clients ship from one commit, so the
  protocol is never versioned (`decisions/server_protocol_is_unversioned.md`).
- **Calls.** After the greeting every frame is a be-wasm-ops frame
  (guides/type_modules.md, "Operations and WIT") of the WIT package
  `be3:server` (`crates/be-block-protocol/wit`, generated with `network: true`),
  whose one interface, `server`, has five calls and one stream beside `ping`
  and `device-login`: `read(organization, query, follow)`,
  `unfollow(organization, topic)`, `write(organization, ops)`,
  `send(organization, target, payload)` and `events()`. Each call is an op,
  with no request ids. Every call names its organization: none for what is the
  account's own (its recovery key, contacts, links, mailbox, storage, its
  organizations), else one it is a member of, so one connection works in any of
  its account's organizations at once; there is no open organization. A query
  is answered by its case of `page`, an op by its case of `done`. Client code
  is `be_block_protocol::read::<KeyPage, _>(&connection, Some(organization),
  Query::Keys { .. }).await??` (`write` for one op, `write_all` for a batch
  answered as its first op, `ask` to follow, or the generated
  `client::server::*` for the rest).
- **Writes are batches.** A `write`'s ops apply in order. More than one, or
  one that makes or moves a block, is
  applied in one SQLite transaction (`ServerStore::batch`): every store job its
  ops make runs on the database the batch holds (a task-local lends it to
  `with_database` and `with_reader`, one job at a time, given back even when
  the job panics or its caller is dropped, and `write_transaction` nested in it
  is a savepoint), so a refused op rolls back every op before it, with the
  cached graphs, policies and object counts read again (`forget_cached`). A
  batch let go of before it ends (its connection closed) rolls back on the
  blocking pool (`Ending`), and a transaction ever found open on the writer
  outside a batch is rolled back and logged. While a batch runs, `read_graph`
  answers nothing, so no access check reads a graph change not yet kept (a
  download's check then waits on the database lock). The
  refusal names the op (`write-error.index`). Each op's rule is applied as it
  comes, inside the batch, so an op sees what the ops before it did (a block
  made, then published), and an op's own checks (a rotation leaves no block
  below on its old key) refuse the whole write. What must hold over several
  ops is checked once, at the write's end, over everything it wrote
  (`keys::Whole`, `ServerStore::check_whole`), the refusal naming the op that
  asked for it: a block made has the data key it names among the rows the
  write stored (`create-block` carries no keys: `[create-block,
  put-keys(rows)]`), and a move into another scope leaves every key it took
  there opening from the new scope's latest key (`[move, put-keys(rows)]`; a
  move within one scope carries none). So keys have one form, `put-keys`,
  whatever else the write does. The caller is admitted by its
  role as of the write's start, since the hub takes a role change only once
  the write commits: safe, as no op raises its caller's own role, and the
  store's checks inside the transaction see the write's own ops. What an op does in memory
  (pushes, session entries, the hub's roles, connections dropped: `After`)
  waits for the batch to commit and is dropped if it rolls back (the store's
  `usage_changes` and `session_changes` stay immediate: each only says to read
  again); a single op
  takes no batch and does it at once, its store work a transaction of its own
  (and a publish's objects upload outside the database lock). At most
  `MAX_OPS_PER_WRITE` ops a write, and signing out (`revoke-token`) is a write
  of its own.
- **Following.** What the server pushes goes on the connection's one `events`
  stream (a second replaces the first): `tree-advanced` for the trees it
  follows, `session` for the live sessions it is in, `appended` for the signed
  logs it follows, `topic-changed` for the
  other topics it follows, and `paired` for its account's other connections'
  messages. A read with `follow` follows its query's topic first, so nothing
  changed during the read is missed: `tree` its entry (in `WatchHub`),
  `session` joins the block's session (in `SessionRegistry`), `block-log` the
  block's signed log (in `WatchHub`, "Signed logs" below), and `keys`,
  `key-work`, a listing of the roots (`children(root)`, `deleted`, `shared`),
  `storage`, `mailbox`, `contacts` and `mutes` their topic (in the
  connection's `Following`). A topic-changed carries no value: the client reads
  the query again. `unfollow` stops one, and leaves a session. A query with
  nothing to follow refuses `follow`.
- **Who may call what.** Every query, op, target and topic states its
  requirement once, in `query_rule`, `op_rule`, `target_rule` and `topic_rule`
  (`client.rs`): anyone (reading an invitation), the account's own (signed in,
  naming no organization), a member or an administrator of the organization
  named, at least some access to the blocks it names, or a session the
  connection is in. The dispatcher (`Client::admit`) applies it before the
  handler runs, which then checks only what a rule cannot say, under the graph
  lock. A call naming an organization the account is not a member of is
  refused; the first one it may make there makes the connection one that works
  there (`WatchHub::join_organization`), which hears what the account's
  connections there hear (key work, keys, roots) and keeps a member from being
  pushed to (`connected`). The hub caches who each connection is in each
  organization, so whatever changes it goes through the hub: a role through
  `set_role` (the connection stays, admitted by the new role from its next
  call), a removal through `drop_member` (its connections close), a sign-out
  through `leave_account`. A block's agent is refused what reaches past its
  block before the rule (`bound_may_read`, `bound_may_write`,
  `bound_may_send`), and everything while its block is in the trash.
  `every_call_states_who_may_make_it` makes every case of `query`, `op` and
  `target` and fails until a new one is made there too. A block the caller may
  not know exists is refused as `BlockNotFound`, like one that does not exist;
  `PermissionDenied` means it may know it exists.
- **Signing in.** A connection signs in with its greeting, before any call:
  the token it carries, or with none the request's `Authorization: Bearer`
  header (`Credentials::Header`, for `be` behind a proxy that injects it;
  hosting.md, "Proxy account credentials"). The server's greeting answers the
  account and this connection's `client` id (`authenticated`, its token empty
  when it came in the header), or the refusal (`InvalidCredentials`, or
  `NotAuthenticated` with no token at all), and then closes the connection
  (`Client::greeted`). A browser cannot set a websocket's headers, so the host,
  the web build too, signs in with a token in the greeting
  (`Credentials::Token`). `revoke-token` signs out and closes it
  (`Connection::closing`). Signing in through the browser follows RFC 8628's
  device flow over one-shot calls (`http::one_shot`): `POST .../call` carries a
  greeting and one frame, `device-login` (`begin` or `poll`) or the `read` of an
  invitation, which a signed-in connection is refused (`Rule::OneShot`), and is
  answered and closed with no session, so nothing is held
  while a person approves it. `begin` answers a code, a secret, the poll
  interval and the lifetime; a poll sooner than the interval fails `SlowDown`
  and grows it by `SLOW_DOWN`; an approval hands its token out to one poll.
  One-shot calls are limited per address (`MAX_CALLS_PER_ADDRESS` each
  `CALLS_WINDOW`).
- **The server's side** is a `be_wasm_ops::net::Peer` (`Side::Host`,
  `client.rs`). It answers calls in arrival order; work a task finishes later
  (following the payer's storage) comes back to the loop as a `Late`. `WatchHub`
  sends each connection `Push`es, which `Client::push` writes to its events
  stream when it follows what they tell. Slow consumers are
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
  - Connections not signed in are counted apart (`address::Unsigned`), from
    a connection's first byte until its greeting signs it in: at most
    `MAX_UNSIGNED_CONNECTIONS` (a quarter of `MAX_CONNECTIONS`), and
    `MAX_UNSIGNED_CONNECTIONS_PER_ADDRESS` for one address, counted by
    `unsigned_key` (an IPv6 /48). One past the overall limit lets go of the
    oldest rather than being refused (`Eviction`), so signed-in clients always
    have the rest of the room and a new client still gets in to sign in. A
    connection carrying HTTP posts counts as not signed in until a post is for
    a signed-in session; an HTTP session exists only once its greeting signed
    it in. A one-shot call is never signed in.
  - Pending browser sign-ins and portal form tokens are kept in a
    `browser_auth::Bounded` table, indexed by age, by address and by network:
    one address holds `PENDING_PER_ADDRESS` or `FORMS_PER_ADDRESS`, one
    network (`unsigned_key`, a /48 of 65536 /64s) `PENDING_PER_NETWORK` or
    `FORMS_PER_NETWORK`, the server `LIMIT`, and another lets go of the
    oldest of whichever is full instead of being refused, so one network
    never pushes out everyone else's.
  - Places are taken before a request's body is read: a connection's by its
    peer in the accept loop, a trusted proxy's by the address each request
    forwards, in `http::front` (a pooled connection that carries another
    client's request gives the last client's places back and takes the new
    one's, or answers 429 and closes), which also admits a request that is
    not signed in among `Unsigned` (an S3 request, signed by its URL, is not
    counted there). A connection that is let go of to make room closes.
  - hyper parses every head: one with more than `MAX_HEADERS` headers or
    `MAX_HEAD_BYTES`, two differing `Content-Length`s, or a length that is
    anything but ASCII digits is refused with a 400 (or 431), and `front`
    refuses any `Transfer-Encoding`, so a proxy in front never frames a body
    otherwise than the server and smuggles a request inside it. The first
    head must arrive within `HANDSHAKE_TIMEOUT`, each later one within
    `IDLE_TIMEOUT` (hyper's `header_read_timeout`).
  - Each route caps its body (`tower_http::limit`, refused with a 413 before
    it is read when its `Content-Length` is larger): `MAX_CALL_BYTES`,
    `auth::MAX_FORM_BYTES`, `MAX_BODY_BYTES` for a post, `MAX_OBJECT_BYTES`
    for an object. Bodies grow only as their bytes arrive and stall out after
    `BODY_READ_TIMEOUT` (`tower_http::timeout`); a call's or a form's must
    arrive whole within `HANDSHAKE_TIMEOUT`. A websocket is idle only after `IDLE_TIMEOUT` with no
    bytes read at all (`next_heard`), so a large message on a slow link is
    not cut off however long it takes.

  An address is a client's `Services::client_address`: its peer, or the
  `X-Forwarded-For` hop nearest it when the peer is in `trusted_proxies`
  (loopback by default, `--trusted-proxy` for more). Per-address limits count
  an IPv6 /64 as one address (`address::address_key`, `address_key_text`).
- **Round trips.** Calls on one connection are answered in order and both
  transports send whatever is queued together, so independent requests cost one
  round trip; only a chain where each needs the last one's answer costs more.
  A batch is for what must apply together, not for speed.
- **Publishing** is a compare-and-swap on the head: a stale publish comes back
  `Rejected` with the head to merge against. A head moves only to a commit that
  descends from it: a publish lists the expected head among its parents, or it
  is refused (`InvalidRequest`); one onto no head may list any (a merge onto a
  block with no commit yet carries the commit refused before it). These are the
  parents the client declares, not the commit's own, which the server cannot
  read: a guard against a buggy or stale client, not against an editor who
  lies. A merge's other parent may be a commit the server never held, carried in. It carries what changed since the
  expected head: an `ObjectChange` (objects added and removed; the server
  lists the commit's objects in `commit_objects` from the expected head's) and
  a `ReferenceChange` (references removed, and added at their index in the new
  list; a moved one is both). It writes the head, refcounts for its new
  objects and only the edges that changed: `block_edges.position` is spaced
  (`EDGE_SPACING`), so a reference put between two others takes a position
  between theirs and the rest stay. A first commit (no expected head) lists
  everything, replacing the references its block was made with. A publish
  with no commit sets the metadata and the reference change alone while the
  head is `expected`, which it leaves the head (`describe`), answered with the
  block's summary; another head rejects it with that head, as a stale commit
  is (`be_block_protocol::Described`). A client sends
  one only for a block with no content yet: once there is content, its
  metadata goes in the commit.
- **Storage** is charged to each block's payer (`blocks.payer`): its parent's
  payer, or at the top its author (an agent's creator), counting each distinct
  object once per payer and every version a block keeps. A move re-charges the
  moved subtree to the payer of where it goes (`follow_payer`), the mover at
  the top; the trash changes nothing
  (`decisions/storage_is_charged_to_the_trees_owner.md`; `store/usage.rs`). An
  account may store `ServerConfig::account_quota` bytes, and an organization
  its settings' `storage_quota` bytes; an upload, publish or hold that would
  pass either is refused with `OverQuota` before anything is written, and
  nothing that frees space is ever refused. Each object `reserve-uploads` reserves
  (`reserve_uploads`) or `publish` carries is a row in `object_uploads` (per
  organization, account and object, with its payer) charged to the payer as
  `pending`, once per payer and object, until one of the uploader's own
  publishes or holds uses it, which deletes the row; `collect_uploads` deletes
  rows older than `upload_grace` (and the object, when no block holds it), when
  the server starts and every `UPLOAD_SWEEP_INTERVAL`. A reservation is renewed
  each time its URL is signed, and the URL expires well inside the grace, so no
  upload can land after its reservation was collected and leave bytes nothing
  names. Freed objects are deleted after their transaction commits but while
  the database is still held (`Objects::remove_freed`), so no upload of the
  same object is reserved in between; a delete that fails is logged, not
  returned, since the call's change is already committed. Counters are kept
  incrementally:
  `object_sizes`, `account_objects` (per payer and object, how many of its
  blocks hold it), `account_usage`, `organization_usage`, and `block_sizes`,
  which the cached `BlockGraph` mirrors as `BlockNode::size` and `tree_size`
  (`add_size`, `set_parent` and `remove` carry a subtree's bytes up its
  ancestors). `BlockSummary` reports both; `query.storage` answers the
  account's `held`, `pending` and `quota`, and following it tells
  `topic-changed(storage)` whenever the payer's `account_usage` row changes to
  other than the connection last read (`ServerStore::usage_changes`, a
  broadcast of payers each connection following its own filters; one that falls
  `USAGE_CHANGES_QUEUED` behind reads again). An object no block holds any more stays while an
  upload row names it (`forget_objects`), so a free never races an upload that
  is about to write the same file.
- **Objects** are served only to a member who can view a block that holds them
  (by a commit, `commit_objects`, or by `HoldObjects`, `block_objects`); anything else reads as
  missing. A `lasting` hold (a repository's snapshots) lasts as long as the
  block; an `until-committed` one (`put-part`) ends when a commit of the block
  holding the object publishes (`release_committed`), or once `upload_grace`
  passes after its last hold while the block's journal is empty
  (`collect_holds`, run by `collect_uploads`),
  uncharging the block and freeing what nothing else holds. A merge commit holds its extra parents' objects, so a stash that was
  never a head stays readable. A block may hold or publish an object only if
  its caller has a proven, unused upload of it there (`object_uploads.proven`)
  or may already read it (`holdable` in `blocks.rs`), and its bytes are
  confirmed (below); `MissingObjects` answers by the same rule, so knowing a
  hash neither reads it nor tells whether someone else stored it. An upload
  proves the uploader has the bytes only when they are the first to arrive or
  were carried in the call: sending bytes the store already holds changes
  nothing the server can see, so a reservation of an object already stored
  (`reserve` first confirms any upload of it that arrived) is unproven.
- **Live sessions** are keyed by organization and block (`watch::Place`): a
  block id is the client's choice, so another organization can hold a block
  with the same one. A connection joins a session by reading `session(block,
  tag)` with `follow`, which answers its client id, the membership, the journal
  and whether it may edit; it is in one session per block (joining again leaves
  and joins), hears the session as `events.session` until `unfollow` or its own
  `left` entry, and sends to it with `send` (`ordered` submissions, `ephemeral`
  relays, `claim`, `host`). Joining needs `Join`, and claiming a session's
  lease needs `Edit` (a viewer's claim is refused, and the replica host ignores
  the refusal). Each answer and `session-event.host` name the host and its
  account (`SessionRegistry::host`): the lease holder only by a claim
  (`Membership::claimed_holder`), never the first editor the lease falls to
  with none, since a device claims as its host component starts
  (plans/hosted_blocks.md, step 4d2); the lease, who seals, is unchanged.
  `query.log` then proves the host to a reader below Edit. The claimant is
  not told of its own claim.
- **Join members** (plans/hosted_blocks.md): a caller below `View` joins as one
  (`joined-session.joined-as`, `member-kind.join`). It is in no membership, so
  no participant learns of it; it hears only `session-event.host` and what the
  holder sends it, and the holder alone hears it come and go
  (`session-event.joiner`, and every Join member already there when the lease
  moves). Its only send is `host`: the host channel, which a viewer or editor
  uses too, so no member reads another's messages to the host. The holder
  sends to one member it names; anyone else sends, naming none, to the holder
  (`SessionRegistry::host_route`); the rest is refused, and a send to a
  session no one holds fails. What an account sends the holder, across its
  connections, is limited per session to `limits::HOST_MESSAGES_PER_SECOND`
  over a burst of `HOST_MESSAGE_BURST` (`SlowDown`), each at most
  `MAX_HOST_MESSAGE_BYTES`. The payload is sealed end to end
  (guides/blocks/keys.md, "Hosted channels"). `query.log` proves, on a path
  to a reader below Edit, the account holding the lease of each session the
  connection is in whose block is the log's block or lies below it, and on no
  other block's log. Beside what
  the plan grants it, a Join member learns, as `KnowExists` and View members
  already do, its no-access summary's `changed` and the length of the log.
  `recheck_sessions` takes out a Join member that lost `Join`, and a
  participant that fell to `Join`, which joins again as a Join member. An
  `append-log(ban)` (Edit) appends a ban record (`access::append_ban`); joining
  below Edit is refused while the block's log bans the caller
  (`access::banned`, read again once a Join member is registered), and a
  banned Join member or viewer in the session is taken out; a ban that cannot
  be read bars.

  After a grant, a revocation, a move or a role change,
  `recheck_sessions` checks each participant below the block again: one that
  lost `View` (a grant removed, the block trashed, or a move out of view)
  leaves, and the trees it follows advance, where it reads that the block left
  or that it may no longer read the tree; a change of `Edit` is written into the
  session's order. Joining checks `View` a second time once registered, so a view
  lost in between does not stay. Trashing a block rotates no keys.

### Where objects live

`ServerConfig::bucket` chooses the store (`objects::Objects`), which the store,
usage accounting, collection and backups all go through: with none, a
`FileStore` in `DATA/objects`; with a `BucketConfig`, an S3 bucket
(`objects::S3Store`) behind a cache (`objects::Cached`): a `FileStore` in
`DATA/object-cache` opened with `FileStore::open_cache` (never synced), holding
at most `cache_bytes` and dropping the least recently used object first. A put
goes to the bucket, then the cache; a read the cache misses is fetched and
cached. Only the S3 API is spoken (decisions/objects_live_in_s3_compatible_storage.md):
path-style, one object per key, the key being the hash's lowercase hex, flat
at the bucket's top. `objects::Bucket` presigns with `rusty-s3`:
`presign_get(hash, ttl)` and `presign_put(hash, length, ttl)` (both on `s3::presign_get` and `s3::presign_put`, the one presigning the endpoint uses too), whose
`Presigned::headers` (`Content-Length`, `x-amz-checksum-sha256`, the base64 of
the hash) are signed, so the bucket takes only that object; the server's own
requests are presigned the same way and sent with `ureq`.

A bucket is a round trip away, so a request asks it about an object at most
once: `object_sizes` names every object from when its upload is reserved
until `forget_objects`, with the reserved length, and answers usage's sizes.
Bytes arrive after the reservation, so an object counts as stored
(`store::stored`, its `confirmed` column) only once `confirm_uploads` saw it
in the store with that length (`Objects::len_many`, a HEAD each on a bucket):
`publish`, `hold-objects` and `reserve-uploads` confirm the caller's own
unconfirmed reservations among the objects they name first, and a held
object is confirmed already, so downloads ask nothing. Carried objects are
confirmed when written (`put_objects`). Reads and writes of several objects go
`objects::PARALLEL_READS` at a time (`Objects::get_many`, `put_many`,
`len_many`). Tests run an S3 server in process: `be_server::TestBucket` (`s3s`
over `s3s-fs`, on a runtime of its own), which clients' tests use too.

### Organizations and access

The tenant is an organization (everything keyed by `organization_id`): one
block graph, its members and administrators. Its `kind` is `company` (made by
`CreateOrganization`) or `personal`, the server's one shared organization
(`ServerStore::personal_organization`), which registering joins as an editor
and nobody administers. An organization always keeps one administrator;
changing a member closes their connections (`WatchHub::drop_member`) so they
reconnect with what they now are.

Access is per member (`Visibility` in `blocks.rs`, `be_block_graph::Access`:
`None < KnowExists < Join < View < Edit < Manage`): administrators manage everything
but owner-only blocks, which they only know exist (guides/settings.md, Owner only);
anyone else gets the best grant on the block or an ancestor (a lower grant
never reduces an inherited one), and `Manage` on what they authored while they
can view its parent (or it is `Root` or `Detached`), so a member taken off a
workspace loses what they wrote inside it. Settings blocks and edit kinds take
their access from their parent: guides/settings.md, "Access". A search index has
exactly its parent's access and takes no grants (guides/blocks/host.md, "Search").

- A block someone can view that references another gives them `KnowExists` on
  it, one level only: they learn its id, content type and access
  (`BlockSummary::no_access`) and are served none of its keys or objects, and
  may not watch it or join its session. Anyone can give themselves
  `KnowExists` on any id by listing it in their own block's references, so it
  never grants more than that summary.
- `Join` (plans/hosted_blocks.md) is granted like any level and, like
  `KnowExists`, reaches only its own block: its member reads the same summary
  (the block is listed in its `Shared`), is served no keys or objects, joins
  the block's session as a Join member (below, "Live sessions") and is served
  the block's log as a proof path of its own right and of the lease holder's.
  A viewable block whose parent is not viewable is an entry point (Shared with
  me).
- Moving (`move`) needs `Edit` on the block and the new parent, and
  `Manage` to move a workspace or into another workspace, or when the move
  would raise the mover's own access (`BlockGraph::move_refusal`). It carries
  the mover's parent record (`placement`), which the server appends to the
  block's log as for a grant (`access::append_parent`); `create-block` carries
  the creator's after the root.
- `SetAccess` needs `Manage`, and a grant above `None` must name a member who
  is a contact of the granter (guides/blocks/keys.md). A share link stands in for the contact
  check: its opener signs its own grant with the link's key (guides/blocks/keys.md, "Share
  links"). Who a link may admit is the organization's `link_sharing` policy
  (`OrganizationPolicies::link_sharing`: members, the default; anyone with an account, only
  while it shares outside; or no links), checked as a link is made and again as it is read and
  opened. The share dialog reads the policy from the organization's settings as it changes
  (be-sdk-shell's `Share::link_choices`), so it offers what the policy allows. An account a link
  admits from outside joins as a guest (`OrganizationRole::Guest`), which `is_member_of` does
  not count, makes no blocks, agents or links and grants nothing (`client::guest_may_write`),
  and leaves once it holds no grant (`share_links::drop_idle_guest`, from `write_grant`, so
  every path that removes a grant). A guest reads none of the organization's settings and is
  served its log as a proof path, so it learns no member. Narrowing the policy refuses new links
  and openings only: guests and grants made through links before stay. Anyone may revoke their
  own grant. `ListAccess` never lists the rest of the membership. `query.names`
  (`list_names`) tells anyone who may view a block who else may view it, its
  author among them only while the author may view it too, each with the display name the account gave the server and
  nothing more (no email, role or grant): what an editor names a member by
  when they are not the reader's contact (guides/apps/host_services.md,
  "Naming a block's members"). It reads `audience`, so a block open in a
  workspace lists every account that views anything inside the workspace
  (`inside_viewers`) and may view the block. A block's agent may not ask it, and a guest is named only
  itself, so it learns no member. Carrying an
  owner record, it moves ownership (`transfer_owner`): only from the owner, to
  a member who is their contact, and the block counts against the new owner's
  bound on blocks (`OverQuota`), checked after the rest, so an owner learns
  whether an account is at its bound only of a contact.
- Each block has an access log (guides/blocks/keys.md; table `access_log`,
  `be-server/src/access.rs`), served by `query.log(block)` to its viewers and to
  anyone viewing a block below it (`Visibility::reads_below`), who checks its
  ancestry and the records resting on it there. Its root comes with
  `create-block`, and the server takes it only signed by the caller with the
  keys `recovery_keys` holds for it now, so a good server and a device never
  disagree (guides/blocks/keys.md, "Access logs"). The server checks a record's
  authority from the logs alone, which block is above which by their parent
  records (`access::checked_block_log`), never its own tree. It checks each
  log once: it keeps every log it checked at its head (`access::checked`, the
  most recently used up to `CHECKED_ENTRIES` entries in all, `be_block_keys::lru`),
  and checks an append or a read again only past
  it, unless the stored log no longer reaches that head, when it checks it from
  its root. A company
  organization's log (`organization_log`) is rooted the same way by
  `create-organization`, whose caller picks the organization's id, and served
  to its members by `query.log(none)`. Each is served whole to a caller
  with Edit or above (on the block or one below it, `Visibility::served_whole`;
  on any block, or administering, for the organization's), and to anyone else
  as a proof path (`access::served_log`, `be_block_keys::proof_path`) whole
  only where it proves the caller's right and that of each account that sealed
  it a key (`wrapped_keys.sender`), and every parent record. A block above what
  the caller views, but not viewable itself, is always a proof path, which also
  proves whoever signs below on authority from it (`access_log_authorities`,
  `access::inherited_signers`). A block's creator holds Manage on it, so
  every member who has made a block is served the organization's whole log,
  and with it its member list. A caller served a log whole names the entry it
  holds the log up to (`held-entry`: its place and hash), and is sent only the
  entries after it while the log as stored chains from its root to that entry
  there, else the log from its root (`access-log-page.from`), which a device
  that holds a rewritten prefix refuses; a proof path is always sent whole.

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
walks what the member can see (`Visibility::candidates`),
not the organization, since the personal organization holds every account's
blocks.

### Listings and following trees

A connection reads what it shows with plain reads and follows trees, not
listings; there is no stream per block. A `list` query (a `Listing`: children,
`Shared`, backrefs, references, `Deleted`, `OpenSettings`) comes a page at a
time, in the order of block ids (`ServerStore::list_page`, which works out the
listing's ids and summarizes only the page's blocks): the summaries that fit in
`ServerConfig::listing_page_bytes`, then the next ids in half that more
(`listed.rest`), and `listed.next` when it goes on, which the client passes back
as `after` for the ids past it. The client reads `rest`'s summaries with a
`blocks` query (at most `MAX_SUMMARIES_ASKED` ids), which is also how it reads
one block's summary: a block the caller may not know of reads as none. A block
that stays in a listing is on exactly one page; one that joins or leaves between
pages has a `changed` past the first page's `listed.sequence`.

**Entry points.** Every summary a reader may view names its `entry`
(`blocks::entry_of`): the highest block at or above it the reader views through
every block between, so the top of its tree (a top-level block, or the block at
the top of what lies in the trash) or the block where the reader's view starts
below a parent it may not view (Shared with me, an agent's block, a settings
kind opened to whoever views a block inside its owner). A client follows the
entries of what it shows: a `tree` read with `follow` follows the entry
*before* reading, so nothing changed during the read is missed, and `unfollow`
stops. `WatchHub` keeps, per connection, only the set of entries it follows (at
most `MAX_FOLLOWS_PER_CONNECTION`) and who follows each (`watch::Place`, by
organization and block).

**Telling.** After each change the connection that made it works out the blocks
on its path (`ServerStore::advanced`: each changed block and every block above
it; with a move, the blocks above it before too; with a grant or a move, every
block below it), finds their followers (`WatchHub::followers`) and, for each
that may still view what it follows (`ServerStore::viewing`), calls
`WatchHub::advance`, which sends `tree-advanced(organization, entry, sequence)`
on the events stream. The hub tells a connection once, then nothing more of that
entry until it reads the tree to its end (`tree` answering no `next`,
`WatchHub::read`), and then once more if the tree advanced past what it read
(`Told`): a burst of a hundred publishes reaches a follower as one or two
events, and a follower of a sibling subtree hears nothing. A change is told only
if it is newer than the connection's last read to the end: its connection calls
`advance` after the write commits, so a read can take the change first, and
telling it then would leave the entry outstanding for a read the client, whose
cursor is already past it, never makes. Who changed a block
is not told; a client tells its own publishes from others' by their heads. A
follower that may no longer view what it follows (a grant revoked, a move out
of view) is told once more (`WatchHub::left`) and unfollowed, so it reads the
tree, is refused, and lets go of what it showed of it. A failed read resets
the follow, so the next change is told again (`WatchHub::unread`).

**Roots.** What lies at the top, in the trash and is shared with an account
changes with a grant, a creation at the top, or a move that leaves or reaches
the top or the trash or changes who may know of the block: `topic-changed(roots)`
goes to the connections following the roots of the accounts that could know of
the block before and can now (`Connection::roots`, `WatchHub::roots`), who list
those again.

Every summary carries `changed`, the organization's sequence at the block's last
change (below), and a `list` query answers the sequence it read the listing at
(`listed.sequence`). Replies and events can arrive out of order (a reply
overtakes events queued before it), and `changed` is how a client puts them
back in order: of two summaries of one block, the one with the larger `changed`
is the newer. It is durable, so a summary read before a restart still orders
against one read after it.

### Changes since

`changes.rs` keeps a durable, organization-wide change sequence
(`organizations.change_sequence`), advanced in the transaction of each change
(every graph change runs under the store's one database mutex, so writes are
ordered). Every write that changes what a block holds, where it lies or who may
view it sets the `changed` of what it changed to the next sequence, in the
database and in the cached graph (`BlockNode::changed`): `touch` (create,
publish, metadata and references, and with them the blocks they started or
stopped referencing, whose `KnowExists` moved; a grant, for everything below the
block, whose access moved; a block given a new key) and `touch_moved` (a new
parent, for the block and everything below it). A block that leaves an entry
point's tree leaves a row in `block_departures` for that entry, kept a day:
`entries_above` (the top of its tree, and each block above it that carries a
grant or opens to the blocks inside its owner, the only blocks below the top
where a view starts) before the move, less those above it after, so a move
within one tree is a departure from a shared subtree it left. That is bounded by
the grants on one path, not its depth. A reader away for more than a day misses
what left in between, moved out, trashed or deleted. A departure names the
block to every reader of that entry, a reader granted after it left included
when it reads from a cursor that old; it learns only an id. A size change
(`size`, `tree_size`) moves no `changed`, so an ancestor's tree size is as new
as its last change, and `entry_of` makes O(depth) access checks of O(depth)
each per summary.
`query.tree(entry, after, limit)` asks about the tree of any
block the caller may view (an entry point, for its departures to be whole) and
pages through `blocks_changed` and that entry's departures in one order, oldest
first: the summaries of blocks in the tree now that changed, and the ids of
blocks that left THAT tree and are not in it now (ids of blocks a reader may not
view are named too: a reader of a tree is told that a block left it, except
blocks only their author may read), a cursor, and the organization's current
sequence. A page ends at `MAX_CHANGES_PER_PAGE` blocks of either kind,
`MAX_PAGE_SUMMARY_BYTES` of summaries (so a tree of blocks with large metadata
still pages within a frame) or `MAX_SCANNED` rows looked at; a page that stops
on another tree's change reads on until the next block of this tree in that
change's sequence, or else to its end and returns a cursor that names no block,
so a cursor never carries a block id the caller may not know.

### Limits

`limits.rs` names what one account or connection may make the server keep:
blocks authored per organization (counted in the graph, `BlockGraph::authored`),
organizations created, open contact links, contacts, invitations outstanding and
escrow keys per organization (refused as `OverQuota`); and per connection,
sessions, listings and streams. Names, metadata (held in the graph and sent in
every summary), sealed blobs, pairing messages and relayed session messages
have byte limits (refused as `InvalidRequest`). A commit of a search index may
hold at most `MAX_INDEX_BYTES` (refused as `OverQuota`), since any editor of its
root writes what every viewer's search reads, and no commit's time may be more
than `MAX_COMMIT_AHEAD` (an hour) past the server's clock (refused as
`CommitAhead`), since a far-future time wins every comparison by commit time
and escapes history's thinning by age. `query.server-info` answers the server's
clock, and a peer stamps its commits no later than that plus
`COMMIT_CLOCK_SLACK` (`Peer::commit_time`), so a device whose clock runs fast is
not refused: it keeps the measurement by its monotonic clock, and the offset in
its refs for offline use after a restart, and ignores one that would pull its
stamps back more than a day (`MAX_CLOCK_PULLBACK`). The index cap sums the base
commit's objects in one query and adds and removes only what the publish
changes. A relay is shared by every
participant's queue (`Push::Relayed` holds an `Arc`). A relay named for one
client that has left the session (or never joined) is dropped without an error:
it is best-effort, and the sender cannot know the other left. Each relay reaches
participants stamped with when the server took it (`received`, by its clock).
Any participant may relay (view access, `rule`): relays are presence and channel
messages, while which head holds which number comes from the server
(`Push::Sealed`). Editors are trusted with the block's content and session.

**The order.** `submit` needs edit access and a build the session speaks
(`Membership::speaks`): the server numbers the submission in the block's journal
(`journal::submit`, one write transaction), stamps it with when it took it, and
sends it to every participant, its author too (`Push::Sequenced`, sent after the
entry is kept, so a connection joining meanwhile reads it in the journal or hears
it here). Joining registers the participant before it reads the journal. Two
submissions numbered at once may reach a participant out of order; replicas hold
what arrives early. The server never opens a payload: it numbers sealed bytes.

**Membership.** The server writes who is in each session into its order:
joining, leaving, a change of edit access while joined, and a claim (`claim`,
from a participant with edit access) are membership entries, each stamped with
the order's last number when written (`after`: it falls between that submission
and the next) and counted (`index`). They are kept in memory only, never in the
journal, so numbering stays gapless and churn never fills `MAX_JOURNAL_BYTES`.
The registry (`sessions.rs`) writes them and sends them and the order's entries
under one lock, so every participant hears them in one order (`Push::Membership`,
`Push::Sequenced`); a joiner gets the membership as of the number it joins at in
`joined` and not its own entry. Entries apply in index order, and a participant
that finds a gap joins again. A session that starts counts on from the highest
index the registry wrote (`Sessions::issued`), so an entry of an earlier session
of the same block that arrives late (its own leave, say) never follows on from
the new one's. The holder is derived from them by one function,
`Membership::holder`, which the server and every replica use: of the
participants with edit access that run the session's build, the last claimant,
else the earliest joined. The server applies an entry as it writes it, and a
replica as it hears it, so a replica lags the server's view by its stream, which
only means two may seal at once (below). So a session of viewers alone has no
holder, one of another build never holds it, a holder that loses edit access
loses it at once, and a claim takes it at once (two devices that claim in turn
pass it back and forth). After a grant, a move, a revocation or a role change
(`append-log`, `move`), `recheck_sessions` writes the changes of edit access. Only the server
writes leaves: when a socket closes, and when a connection goes silent past
`IDLE_TIMEOUT` (50 s), as `leave_all` runs on any disconnect; until then a
silent holder stays the holder. `IDLE_TIMEOUT` is be-block-protocol's `CLIENT_SILENCE`, the
longest a live client goes without sending (twice its `PING_INTERVAL` and its `PONG_WAIT`), so
no live client is given up. The registry indexes sessions by client, so
leaving them all costs what that client joined, and forgets an empty session.
After a restart every session is empty and rebuilds from joins.

Each block's journal (guides/blocks/client.md, "The journal") is two tables,
`block_journals` (tag, run, base, the order's last number and barrier) and
`journal_entries` (an id unique per block), written by `be_server::journal` and
deleted with the block. It is kept whether or not anyone is in the session, so
what nobody sealed waits for the next holder. A seal trims it: `publish` with a
`journal-seal` whose base names the commit published trims the journal to the base only
if the publish wins the head's compare-and-swap, in the same transaction
(`journal::rebase`), and every participant hears which head holds which number
(`Push::Sealed`). Content never rested on the lease, since every replica applies
one order: of two devices that both take themselves for the holder, the
compare-and-swap takes one, and the other is rejected and trims nothing, so a
trim never outruns its commit, and a sealing publish needs no holder check. A
trim with no new commit (the last one out restarting the session, a new build
starting the journal over: a publish of the head on itself) needs the head to be
the base's commit and the caller to be the holder at the order's tip, by the
server's derivation, checked with the registry held while the trim is written
(`SessionRegistry::holding`). A seal names the build that made it and the run of
the order it seals (`journal-seal`: how many times the session had started
over, `status.restarts`), so one the outbox sends before its replica joins again
still trims, and one that is stale trims nothing: of an earlier run (sequence
numbers are reused once the order starts over), below the journal's base, past
the order's last number, or with a new commit by another build than the
journal's. A stale seal's publish still goes through on its compare-and-swap,
untrimmed and untold; no publish is refused over its seal. Only a later run's
base, with nothing numbered past the journal's base, goes back, starting the
order afresh (`journal::Trimmed::restarted`, which alone moves the registry's tip
back). A submission is
refused while the journal holds another build's entries, and bounded by
`MAX_JOURNAL_BYTES`; a `blocks` read with `contents` answers journals while they fit in a
quarter of a frame.

### Signed logs

A block may have a signed log beside its content (plans/signed_logs.md;
`log.rs`): entries the server numbers, stamps and hashes into a Merkle tree,
which no commit changes. Content is what anyone with Edit may rewrite; a log
is what its authors add, and only they (or a manager) may take back.

- **Tables.** `block_logs` holds each entry (place, id, author, arrival, the
  hash of its sealed payload, the signature, and the payload until it is
  redacted), `block_log_nodes` every perfect subtree of its tree by level and
  position, and `block_log_heads` the log's size, root and charged bytes. The
  tree is RFC 6962's (`be_block_keys::merkle`, which access logs may share,
  plans/deferred/proof_paths_with_inclusion_proofs.md): an entry's leaf is
  `be_block_protocol::LogEntry::leaf` (place, arrival, author, id, payload
  hash, signature, not the payload), so a redacted entry still verifies. A
  proof for any page or any older head is O(log n) lookups of perfect
  subtrees (`merkle::subtree`), whatever the log's length.
- **`append`** (Edit) numbers each entry next and stamps it with the
  server's clock, never earlier than the entry before, in one transaction
  that appends to the tree from its right edge (`merkle::Frontier`, read in
  O(log n)). An entry whose id the log holds is answered where it went
  (`logged`), so an outbox resends until it hears the answer; another entry
  (another author or payload) under that id is refused. Limits:
  `MAX_ENTRY_BYTES` each, `MAX_ENTRIES_PER_APPEND` and `MAX_APPEND_BYTES` an
  op, `MAX_ENTRIES_PER_MINUTE` per account (`Services::log_entries`). Entries
  are charged to the block's payer, in its size too (`charge_log`; a move
  carries them to the new payer, `follow_payer`), and refused past a quota.
  The server checks no signature: devices verify on their own, and an account's
  device not yet told of its new phrase signs with the old key
  (guides/blocks/keys.md, "Verifying").
- **`redact`** (View, then the entry's author or Manage on the block) drops
  the payload and uncharges it, keeping the leaf, so the head and every proof
  stay as they were. Dropped, not wiped: SQLite may keep the old bytes in its
  WAL and free pages until they are reused.
- **`query.block-log(block, from, limit, known)`** (View) answers a page from
  `from` (at most `MAX_PAGE_ENTRIES` and half a frame), the head, the range
  proof tying the page to it (`merkle::range_proof`), and with `known`, the
  size of a head the caller holds, a consistency proof from it
  (`merkle::consistency_proof`). Followed, the connection hears each append as
  `event.appended`: the entries, the size before them and a consistency proof
  from it, so a follower holding that head takes them with no round trip. The
  hub keeps who follows each log (`WatchHub::follow_log`), and an append tells
  only those who may still view the block (`Connection::logged`).
- **Commits name a log head.** `new-commit.log-through` names the head of the
  block's log a commit was made on, and a publish naming a head the log never
  had (past its size, or another root for that prefix) is refused
  (`log::holds_prefix`). The server only checks it; what a commit says of the
  log is in its sealed content.
- `log::forget` drops a block's log; nothing calls it until blocks are deleted
  for good.

**Answers that fit a frame.** A summary carries metadata of up to
`MAX_METADATA_BYTES` and none of the block's references: a reader takes those
from the block's content (its commit's `references` and its parts', or its
metadata's for a block with no content yet), which it decrypts, and the server
keeps them only for liveness (`held`), `KnowExists` and backrefs, so one
summary fits a frame many times over (`LARGEST_SUMMARY_BYTES`, held by a const
assertion). A block references at most `MAX_REFERENCES` blocks (as many as an
account may author, so a folder of all of them still fits; refused past it by
`create-block` and `publish`, whose adding that many still fits a frame, and
`publish` also refuses a `ReferenceChange` `between` never makes, naming an
added reference twice or with indices that do not increase; `apply` is one
pass, and moves an added reference the list already holds, since a client
diffs against its own view). The server keeps their order
(`block_edges.position`) for that diff alone: a `references` listing comes in
the order of ids, as every listing does.
Answers of many summaries bound them by encoded size
(`be_block_protocol::encoded_len`), not by count, and always take at least one,
so a summary larger than the budget goes alone: `list-blocks` and
a `blocks` read as above; with `contents`, it answers the first blocks whose
summaries fit in `SUMMARY_BYTES` and the client asks again for the rest, with
its objects and journals in what is left beside them (`contents_room`:
`CONTENTS_BYTES` and half that, or less beside a large summary);
`tree` ends a page at `MAX_PAGE_SUMMARY_BYTES`, and one that reads on
through another tree's change sequence stops at the next block of its own;
`query.keys` pages by key id (`ServerConfig::key_page_bytes`,
guides/blocks/keys.md). Each budget, with the largest summary and
`FRAME_ENVELOPE_BYTES`, is held under `MAX_FRAME_BYTES` by a const assertion.
`query.history` still answers every entry kept at once. Code that walks a block's references must stay linear at `MAX_REFERENCES`:
`BlockGraph::set_references`, `relinked`, `ReferenceChange::apply`, the
hub's `Links::hold` (`BlockGraph::refers`) and be-host-stack's `Graph::holds_in` use sets, not
`Vec::contains`.

Nothing the store does blocks the runtime's threads. Its SQL runs as owned
jobs on tokio's blocking pool, one at a time under the database lock:
`ServerStore::with_database(move |store, database| ...)`, and `with_graph`,
which adds the organization's graph. A job captures what it needs by value
(the store is a cheap clone) and runs to its end even when its caller is
dropped. Uploads are written and synced, and objects read, on the blocking
pool too, outside the lock (`put_objects` after it reserves the quota,
`confirm_uploads`, `get_objects`). Whether a reader may download objects
(`may_read_objects`) is answered from a read-only connection beside the writer
and the organization's cached graph, read under its own lock, so downloads
wait neither for the database lock nor for other organizations' calls; a
graph job that may have left its graph apart from the database marks it
invalid. In tests, `store::held::hold(organization)`
keeps an organization's next graph job waiting until released. A connection
resolves its token again only after `ServerStore::sessions_changed`, not on
every call and push (`Connection::signed_in`), so every path that deletes or
changes an app session calls it.

### Accounts

An account has an opaque id and a set of **identities** (`identities`): its
email, providers (`oidc`) and passkeys (`passkey`). Proving any one signs in
(decisions/accounts_sign_in_with_identities_the_operator_configures.md); the
last one that still signs in (`providers::signs_in`) can't be removed. `accounts.email` is the account's verified email, if it
has one; an account made through a provider may have none, and then the
protocol's `email` is the empty string
(decisions/an_account_with_no_email_has_an_empty_one.md). Sign-in stays in the
browser portal (`/auth`, `portal.rs` and be-server-portal-ui), so no client
needs code of its own for any method.

- **Emailed codes** (`store/codes.rs`) sign in, register and recover ("I can't
  sign in"). Recovering takes the account back from whoever else may hold it
  (`recover_account`, then `delete_agents_created_by`): it ends every other
  session and pending email change and every session of the agents the
  account made, in one transaction, removes every way to sign in but the email
  it proved, then deletes those agents and tells the account in one mail and
  one notice what went. Grants signed to a deleted agent are
  left for a device of someone who manages the block to revoke. Codes are eight letters with no look-alikes,
  good for ten minutes and five wrong tries, belonging to one attempt whose
  secret the asking browser holds in the `be3-attempt` cookie, so it works
  nowhere else; a new request for the attempt ends its older code. The mail's
  link (`/auth/enter?attempt=&email_code=`) fills the code in only in that
  browser, and elsewhere shows it and signs nothing in. The cookie is sent again
  with every request that uses it, so it lasts as long as its latest code. Mail
  goes to the server's log (`mail::LogDelivery`; plans/deferred/mail_delivery.md).
- **Mail limits strangers can't spend.** Requests are capped per address (an
  IPv6 /64) and per network (a /48), and an email's budget counts only those
  from addresses that never signed in to its account, whether or not it has
  one, so strangers can't stop its owner nor learn which emails have accounts by
  running it out; a request for an email with no account leaves a row that
  costs the same and sends nothing. The remembered addresses live in the
  server's memory (`store/remembered.rs`), so a restart forgets them and the
  owner's first request after one counts against the budget like a stranger's.
- **Registering** needs an invite code the operator made
  (`store/invite_codes.rs`), checked when the code is asked for and spent when
  it is entered and the account is made; one invite has at most
  `REGISTRATION_CODES_PER_INVITE` registration codes outstanding. Registering
  by code refuses an email that has an account, which tells a stranger it is
  registered: behind an invite code that leak is small, and mailing "you
  already have an account" is the alternative if registration opens.
- **Fresh proof.** Changing the email, adding or removing an identity,
  ending any session but the browser's own, and approving a device's code
  need a proof in the last few
  minutes (`codes::FRESH_PROOF`, `sessions.proved`): a code to the account's
  email, or a sign-in with one of its providers or passkeys, which signing in
  by code, provider or passkey, or recovering, also gives. A new email's
  verification link (`email_change_link`) is capped per email, per address, and
  globally; only a signed-in account with a fresh proof asks for one, so
  anonymous traffic can't reach the cap (`store/portal.rs`). The link confirms
  only in a browser signed in as the account that asked for it
  (`confirm_mail`), or an account asking for a stranger's address would have
  the stranger's click attach it; opened anywhere else it confirms nothing and
  stays unspent. A browser signed
  in longer ago is asked to sign in again before it approves, and signing out
  keeps the code, so the sign-in returns to the approval.
- **App tokens** expire `APP_SESSION_IDLE` (90 days) after they were last
  used; `resolve_token` renews one it accepts, writing at most once a day, so a
  device or agent in use keeps its token and a forgotten one stops working.
- **Notices.** Removing an identity, adding a provider (from the settings page
  or by a link code) or a passkey, and a provider moving the account's email
  each mail the account's email, the old one for a move. Removing the email
  identity also clears `accounts.email`, and an account with no email never
  gains one from a provider it already has, since its owner may have removed
  it. While mail goes only to the server's log, every account is also told of
  an identity added or removed, or its email changing, by a notice in its
  mailbox (below; `store/identity_changes.rs`). be-host reads them at
  sign-in, so they show before any organization is shown, and one made while
  no device is signed in arrives at the next; while an organization is shown
  its stack follows them as they come. The portal's settings page lists the
  last thirty days of changes, which nothing hides.
- **The mailbox** (`store/mailbox.rs`; `query.mailbox`, `delete-message`,
  `topic.mailbox`) holds what an account is told while none of its devices
  may be connected, each `message` kept until a device of the account deletes
  it: the server's notices (`letter.notice`, from no one), an accepted link's
  card (`letter.accepted`, from the accepter, left by `accept-link`), a
  contact's key change (`letter.key-change`, from the contact, left by
  `set-recovery-key`), and the account's records of contacts made
  (`letter.contact`, sealed by the account for itself, left by `accept-link`
  and `create-agent`) or removed (`letter.removed`, left to both by
  `remove-contact`), and what it pins for an organization (`letter.pinned`,
  left by `create-organization`, an invitation's `accept-link` and
  `create-agent`), which a device takes into the account's block
  (guides/blocks/keys.md, "Contacts"). The server leaves each in the
  transaction of the change it tells, so none is lost between the two, and
  tells the account's connections that follow the topic; a device deletes what
  it took, which tells the account's other devices that its block changed.
  Removing a contact deletes what each left the other but a record not yet
  taken, with the links between them, before leaving each its
  `letter.removed`. An account holds at most
  `MAX_LETTERS_FROM_ONE` letters of each kind from one sender (so at most that
  many times `MAX_CONTACTS_PER_ACCOUNT`). Past it, accepting another link is
  refused, but a key change never is, since a new phrase is what an account
  takes after a compromise: the sender's oldest change goes, and the reader's
  device, finding the chain broken, lists the contact as not confirmed
  (guides/blocks/keys.md, "Contacts"). A block's agent reads its mailbox and deletes from it, since its
  contacts' key changes arrive there.
- **Holding notices.** Whoever a notice warns about may hold a session too, so
  in its first `NOTICE_HOLD` (a week) `delete-message` refuses it and a
  device hides it only for itself (`be_host_base::notices::dismiss`); after,
  one deletes it for all. An account keeps `MAX_ACCOUNT_NOTICES` notices past
  their hold, letting go of the oldest; one in its hold always stays. It adds a
  way to sign in at most `IDENTITY_CHANGES_PER_DAY` (20)
  times a day, counted in the transaction that adds it (`claim_identity_add`),
  so nobody pushes a fresh notice out by adding more. Removals are never
  capped, so the owner can always remove what an intruder added.
- **Cookies.** Over HTTPS the portal's are `__Host-be3-account` and
  `__Host-be3-attempt` (Secure, `Path=/`, so no sibling subdomain can set or
  shadow them); over plain HTTP, which only a server on this machine serves,
  the bare names scoped to `/auth` (`http::auth::PortalCookies`).
- **Secrets.** Every token, cookie, link, form token and HTTP session id the
  server hands out is `new_secret()`, 256 bits from the operating system in
  hex; codes people type have their own alphabets. Of those in its database
  it keeps only the hashes, each kind under its own domain (`store::hash_token`, `TokenKind`),
  so a secret of one kind never matches a row kept for another.
- **Operator login links** (`store/login_links.rs`, `be-server login-link`)
  sign in once within `LOGIN_LINK_LIFETIME` as recovering does, and for an
  email with no account make it, with no invite code. A link by `--email` is
  the operator vouching for that address, which the recovery keeps; one by
  `--account` vouches for none, so it removes the account's email too, which
  the command prints, and its owner adds a way in again; `/auth/link` shows the
  link with a button, so a mail scanner opening it spends nothing. Using one
  needs neither `--allow-login` nor `--allow-registration`, so it is the way in
  on a closed server, on one whose log is not to be read, and for an account
  stranded by a provider the operator removed. The command prints the link on
  its standard output, never to the server's log; only the link's hash is
  kept.
- **The flags.** `--allow-login` lets anyone sign in, by any identity, and with
  it `--allow-registration` lets anyone make an account, by any identity, with
  an invite code. They gate signing in, not using a session: without them a
  browser already signed in still changes its settings, confirms an email
  change (`/auth/verify`) and signs out.

**Providers** are OpenID Connect issuers the operator lists (`--oidc-providers`,
`oidc::parse_providers`): one tenant's issuer each, and only those marked
`authoritative_email` (Google's preset) have their verified emails trusted.
`oidc.rs` does the network half with the `openidconnect` crate: discovery,
read when first needed and kept an hour, one read per provider at a time, and a
failed read held for half a minute; `/auth/oidc/{id}/start`, a GET link since
the portal's CSP has `form-action 'self'`, which checks the caps on begun
sign-ins before it calls the provider, gives the browser a fresh `be3-attempt`,
keeps the request's PKCE verifier, `state` and nonce in `oidc_states` bound to
it and redirects; and `/auth/oidc/{id}/callback`, which takes that row back
once, in that browser only (opened in another, it leaves the row for the
browser that began it), exchanges the code and checks the ID token in full
(signature against the provider's keys, read again at most once a minute when
a token's key is unknown, `iss`, `aud`, `azp`, expiry, nonce). `store/providers.rs` does the rest.
An identity's subject is `"{issuer} {sub}"`, never an email. A sign-in with a
linked identity signs in, and its provider's verified email follows a change
when no other account holds the new one. One with none whose verified email an
account holds mails that email a code (`CodePurpose::Link`), and entering it in
the same browser signs that account in and links the provider: nothing links
or makes a second account on its own. Otherwise it waits in `oidc_pending`
for the operator's invite code (`/auth/oidc/register`) and makes an account,
whose email is the provider's verified one if it has one. A signed-in browser
adds a provider (`?intent=link`, with a fresh proof checked at both ends, the
second time for the same account) or proves with one it has (`?intent=proof`).
Both need the settings page's `check`, a hash of the browser's session
(`providers::start_check`), so another site can't begin one, and ask the
provider for a sign-in just now (`max_age=0`), refusing a token whose
`auth_time` is older than the begun sign-in. A provider identity keeps the
email it gave only while its account holds that email. Begun sign-ins are capped per address
and overall. At start the server warns how many accounts sign in only with
providers it no longer has, or only with passkeys made for another RP ID
(`stranded_accounts`); `login-link --account` lets one in.

**Passkeys** are WebAuthn credentials (`passkeys.rs` with `webauthn-rs`, and
`store/passkeys.rs`) whose RP ID is the portal's origin's host (`--auth-origin`,
or `localhost` in development; an IP address has none, and then no page offers
one). A passkey identity's subject is `"{rp_id} {credential id}"`, so one made
before `--auth-origin` changed is never found, and its `credential` the key and
counter; its user handle is the account's id. They are made
discoverable (`residentKey: required`), so the sign-in page offers one before an
email is typed. A page that offers a ceremony (sign-in; on settings, adding one
when the proof is fresh; on settings and sessions without one, proving with
one) begins it
when it renders and keeps its state in the page's form token
(`browser_auth::Sessions::form`), which is bound to the browser's session,
single use and ten minutes long. The portal's wasm
(`be-server-portal-ui/src/web.rs`) calls `navigator.credentials` with the
page's options (`PasskeyOffer`, WebAuthn's JSON) and posts
`PublicKeyCredential.toJSON()` to `/auth/passkey/{sign-in,proof,add}` in a form
it builds, with the page's token, so the answer meets the portal's Origin check
and spends the ceremony; webauthn-rs checks the challenge, the origin, the RP
ID's hash and user verification. Adding checks the fresh proof again when it
lands, and an account holds at most `MAX_PASSKEYS`, each stored in at most
`MAX_CREDENTIAL_BYTES`. A use is kept only if its counter is past the stored
one, compared in the transaction that keeps it, so two at once don't both
count. A passkey for another RP ID, like a provider the server no longer has,
neither signs in nor counts as an account's remaining way in when another
identity is removed (`providers::signs_in`). Every failed passkey
sign-in gives one answer, so it tells no one which credentials exist. Tests
use a software authenticator (`fake_authenticator.rs`).

The protocol has the browser sign-in's one-shot `device-login` ("Signing in"
above), signing in with a token or the Authorization header in the greeting, and
`revoke-token`; every sign-in method lives in the portal, which
approves a device's code. `Be3-Gateway-Authorization` is left to a gateway in
front.

Contact links and invitations are one table, `links`, and one set of calls
(`create-link`, `query.link`, `accept-link`, `delete-link`; the owner's
block keeps each link's code;
guides/blocks/keys.md, "Contacts"). An administrator invites with a link of
kind `invitation` under an id the inviting client picks, bound to no email: any
account that opens it may accept it, once, before `INVITATION_LIFETIME`, and
joins as an editor; the id reaches the invitee only in the link
(guides/blocks/keys.md, "Inviting"). `create-link` appends the administrator's
invitation record to the organization's log and keeps the key it names
(`invite_key`); `query.link` answers the log's head (`link.head`), and
`accept-link` appends the accepter's member record signed with that key, checked
against it and the log (`access::append_accepted`), in the transaction that
makes them a member, so the server cannot sign anyone in and no administrator
need be online (guides/blocks/keys.md, "Access logs"). An organization has at most
`MAX_INVITATIONS_PER_ORGANIZATION` outstanding. `query.invitation`, the only
one-shot call besides signing in, reads one by its id before sign-in (its
organization's name, the inviter's sealed card and its state) and marks it
opened. The row stays after it is accepted, expires or is revoked
(`invite_ended`, `invite_revoked`, `invite_finished` once the inviter's
device made the accepter a contact) for the organization's invite list
(`query.invitations`, administrators only, which hands each inviter back only
their own sealed code), until `INVITATION_RETENTION` passes or
`MAX_INVITATIONS_KEPT_PER_ORGANIZATION` pushes the oldest ended out. Any
administrator revokes an open one with `delete-link`, carrying the invitation
record that ends its key (`invitation-row.key`).
`CreateAgent` makes a passwordless editor account with a token
(guides/tracker.md), at most `MAX_AGENTS_PER_CREATOR` per creator. Agents are
members: `query.members` names an agent's creator (`created-by`) and, for a
block's agent, its block (`bound-to`), and lists every member to an
administrator but only the agents it made to anyone else; `delete-agent`
deletes any agent for an administrator and one for its creator, with its
account, and an administrator removes a person by signing them out
(`append-log(member)`); each tells the organization's key work as any removal
does.
Only its creator rotates its token (`RotateAgentToken`, which revokes every
older one). One `bound-to` a block is that block's agent (guides/tracker.md, A
block's agent): `Identity::bound` limits what it reaches, `client::bound_may`
what it may call, and `WatchHub::go_live` keeps one connection of it open.
Accepting a link, or creating an agent, makes the two accounts contacts: the
server keeps only that they are (`contacts`), and each account keeps its record
of the other in its own block (`be_block::account_block`, owner-only at the top
of the personal organization, made only there by `blocks::require_account_block`).

### Notifications

Push notifications for members who are not connected (`notifications.wit`,
`notifications.rs`, `push.rs`; plans/deferred/notifications_on_a_device.md for what is left). The
server never reads one: the text is sealed under the block's data key with the
organization, block, sender and the time it was sent (`Peer::notify`, `be_block_client::Notification`).
What it does learn: the block, the sender, when it was sent, whom it mentions,
who was not connected, and from each endpoint which push service each device
uses. Pushes are padded to one length (`push::PADDED_BYTES`), so not even the
text's length reaches the push service.

- **Endpoints.** A device registers one per account and organization
  (`set-push-endpoint`): Web Push (RFC 8030) at an `https://` URL on port 443,
  or an FCM registration token, with the device's RFC 8291 keys. At most
  `MAX_PUSH_ENDPOINTS`, the oldest going first; a member's go when they leave
  the organization, and `revoke-token` with the device's address forgets it in every
  organization. A Web Push URL may only resolve to a public unicast address
  (`push::is_public`: no private, shared or reserved IPv4, and IPv6 only in
  `2000::/3` outside 6to4, Teredo and the IETF's `2001::/23`) and no redirect
  is followed, so a client cannot have the server post into its own network;
  `PushConfig::allow_local_endpoints` (`--allow-local-push-endpoints`) lifts
  both for tests. The host must be a known push service's
  (`push::allowed_host`: FCM, Mozilla, Apple, Windows) or one the operator
  added (`--push-host`, `PushConfig::hosts`), checked at registration and
  again at send, so the server is no relay for POSTs to hosts a member names
  (decisions/push_endpoints_are_allowlisted.md). The device key must be a
  point on P-256. An endpoint is dropped when its service says it is gone (Web
  Push 404 or 410; FCM only `UNREGISTERED`, since a wrong project also answers
  404), or after `MAX_FAILURES` failures in a row over at least `FAILING_FOR`
  (counted per account, organization and address); 429 and 503 count as
  neither.
- **Mutes.** Each account's mode per block (table `mutes`: `all` keeps no
  row, `mentions`, `off`), deleted with the block, the member or the
  organization. The server reads them to leave a muted account out of a
  block's pushes (`mentions` still hears what mentions it), so a muted block
  wakes no phone of theirs; these rows are their one copy, not also kept in
  the account's own block (decisions/mutes_are_read_by_the_server.md).
  `op.set-mute` sets one (`all` unmutes) and tells `topic.mutes` to the
  account's other connections; `query.mutes` lists an account's, which a
  device reads, following them, when it connects and again on that topic
  (be-host-stack `Shared::mutes`, published to apps as `MutesWatch`). The
  list may name blocks the account can no longer view, which is harmless:
  it is not filtered.
- **`target.push`** (a `send`; Edit; at most `MAX_NOTIFICATION_BYTES` and `MAX_MENTIONS`,
  `MAX_NOTIFICATIONS_PER_MINUTE` per account and `MAX_NOTIFICATIONS_PER_BLOCK`
  per account and block) goes to every viewer of the block
  (`BlockGraph::viewers`) but the sender whose mode lets it through and who has
  no connection working in the organization (`WatchHub::connected`: one that made a call naming it, not merely an open socket), with one
  query for the organization's endpoints. Each push carries a `PushMessage`
  (organization, block, the sender's account, which the server vouches for and
  the sealed bytes repeat, and the sealed bytes), encrypted to the device (be-lib-web-push, which
  devices open with too). It goes at high urgency only to whom it mentions.
  The keys a call names (at most `be_block_protocol::MAX_TOLD_KEYS`, digests of what its
  edits announce, `be_block_protocol::told_key`, so the server sees no content) are kept per
  organization, block and account (`told_keys`, forgotten with the block or the member), and a
  call whose keys were all told before is answered without a push (`ServerStore::tell`), so a
  device that sent one and never heard the answer can send it again. Only the newest
  `MAX_TOLD_KEYS` (256) are kept per account and block: a key evicted past those is pushed
  again if it is sent again.
- **Sending.** Pushes queue per recipient account (`Pusher::queue`, at most
  `MAX_QUEUED_PER_ACCOUNT`, and `MAX_QUEUED` over the server) and go one at a
  time in order, its devices in the order they registered, so an endpoint that
  never answers delays only its own account; tests rely on that order to show
  a push did not go. A push about a block that already waits for a device
  replaces it. Each send holds a blocking thread, at most `SENDERS` at once, and `SENDERS_PER_HOST` to one push host
  over the server, so services that never answer cannot take the pool the
  database shares. Past `MAX_QUEUED`, the longest queues lose their newest.
- **VAPID** (RFC 8292): the server makes a P-256 key on first start and keeps
  it in `push_key`; `query.server-info` answers its public half, which a browser
  subscribes with. Each Web Push request carries a token for the endpoint's
  origin, signed with it, whose subject is `--auth-origin` or the public URL
  (warned about at start unless `https://`).
- **FCM** (HTTP v1): with `--fcm-credentials FILE` (a service account's JSON
  key, `push::FcmConfig`), the server signs an RS256 assertion for an access
  token (kept until a minute before it expires) and sends a data message whose
  `data.push` is the same encrypted bytes, base64url, so a device decrypts
  both alike; Google sees only ciphertext. `query.server-info` tells a device whether
  to register through it. Tests run fake push services and a fake FCM and
  token endpoint (`TestPushService`).

## Running it

```
./scripts/buck run //crates/be-server-bin:bin -- --addr 127.0.0.1:9090 --data-dir be-server-data --allow-login --allow-registration
```

Make an invite code with `be-server invite-code create --data-dir
be-server-data` (through the same `:bin -- `), then register at
`http://localhost:9090/auth` with it; every mail, the codes included, is
printed in the server log. `be-server login-link --data-dir be-server-data
--auth-origin http://localhost:9090 --email EMAIL` signs one in without it.
Open the portal at `localhost`, not `127.0.0.1`: passkeys need a host name for
their RP ID. `--oidc-providers FILE` adds providers (guides/hosting.md).
Deployments also set `--auth-origin` (guides/hosting.md). The native host
embeds be-server for a local account (`crates/be-host-server`,
ephemeral port, data under the host's); `./scripts/buck run //crates/be-host:web`
starts one beside the web build.

## Tests

- `be_server::Running` serves a fresh temporary directory on an ephemeral port
  in the current runtime and removes it when dropped: `Harness` in
  `crates/be-server/src/tests.rs` and `crates/be-block-client/src/tests.rs`.
  `Running::lie(account, rewrite)` makes it lie: every block summary it sends
  that account's devices (`query.blocks` and listings) is rewritten
  (`ServerConfig::lies`, `Lies`), so a test shows a client refuses what the
  server alone could say (a block's parent, its current key, its history),
  which editing the database cannot, as summaries come from the in-memory
  graph.
- be-host-stack's `Harness` (`src/tests.rs`) starts the embedded server and the
  host's peer against it; crates above it use `be-host-testing`'s `Harness`,
  which serves the first-party types in process
  (`be_host_stack::testing::serve_in_process`).
- `crates/be-block-fault-relay` sits between a client and a server and can
  `stall`, `stall_at_next_request`, `cut`, `hold_new` or `jam_at_next_request`
  connections, go `offline` (cut them and refuse new ones until it is back), and `count` the calls the client starts (method ids are
  `be_block_protocol::__args::<INTERFACE>_<FUNCTION>`), so a test waits for a
  ping to be sent.
- crates/be-host-tests' `Server` runs one in a host test's process, and be-hosts
  join it as devices of its accounts, each through a relay of its own
  (guides/testing_a_gui.md, 6).
- The graph's tests check access against random graphs
  (`be-block-graph/src/tests.rs`).
