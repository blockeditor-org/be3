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
  that head costs nothing (`Peer::reconcile`).
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

`CollectDetached` (`Peer::collect_detached`) removes every deleted block the
caller may edit with its objects, but nothing calls it yet. Once something
does, content must cope with a linked block that is gone, and collection will
race a new child its parent has not listed yet.

## be-server

SQLite for metadata (`schema.rs`, checked on open by
`be_block_sqlite_schema::ensure`, which only lets a schema grow; the host's
`host.sqlite3` is checked the same way and recreated, signing the device out,
when it does not match), an object store (`objects.rs`, "Where objects live"
below), and postcard frames over a websocket. Where a websocket cannot get through, `POST .../http` carries the
same frames (`http.rs`): the greeting's answer names a session in a
`Be3-Session` header, and each later post carries queued client frames and is
answered with whatever the server has for the client; notifications wait for
the next post. An answer stops taking frames once its replies fill
`OUTBOUND_BYTES`, keeps the rest for the next post, and says so with
`Be3-More`, on which the client posts again at once (with an empty body if it
has nothing to send). The client gives up on a post only after `QUIET_WAIT`
with no bytes moving either way, so a large post on a slow link goes through.

### Objects over S3

Clients move object bytes only through presigned S3 URLs (decisions/objects_live_in_s3_compatible_storage.md):
`objects.upload-urls` takes (hash, length) pairs, reserves them as an upload
(below, "Storage") and answers a presigned PUT per object (`upload-url`: the
URL and the headers to send with it); `objects.download-urls` answers a
presigned GET for each object the caller may read (`may_read_objects`) and none
for the rest. Each names at most `be_block_protocol::MAX_OBJECT_URLS` objects.
Only `publish`'s carried objects and `read-contents`' objects travel inside
frames. The URLs are signed by `Services::transfers`, an `objects::Bucket`:
the configured bucket, so bytes go between clients and the bucket and never
through the server, or with none the server's own endpoint below
`ServerConfig::public_url` (`--public-url`; by default `http://` and the
address it listens on), `S3Endpoint::transfers`. A URL lasts
`ServerStore::transfer_lifetime`, at most 15 minutes and under half of
`upload_grace`.

With no bucket, be-server answers S3 requests itself at `.../storage/objects/HASH` (`s3.rs`, served
by `http/s3.rs` through the `s3s` crate; with a bucket it serves none): path-style, one bucket `objects`,
keys the object's hash in lowercase hex, whatever path the proxy puts in
front (`OneBucket` gives s3s the bucket, so signatures cover the full path;
s3s still checks the first path segment is a valid bucket name, so the mount,
`s3::PATH`, is `storage/` and not `s3/`).
Only GetObject (with Range), HeadObject and PutObject are served; PutObject
requires `x-amz-checksum-sha256`, stores only a body whose hash is both the key
and the checksum, and answers once the object is synced. Every request is
SigV4-signed, in practice presigned, with one credential pair derived from
the server's own secret: `ServerStore::s3_endpoint` gives an `S3Endpoint`,
whose `bucket(server_url)` and `credentials()` feed `s3::presign_get` and
`s3::presign_put` (which signs the length and checksum, so the URL takes only
that object; `rusty-s3` signs). Bodies are read whole within
`BODY_READ_TIMEOUT`, up to `MAX_OBJECT_BYTES`. Answers carry
`Access-Control-Allow-Origin: *` and `Cross-Origin-Resource-Policy:
cross-origin`, and preflights are answered, so a cross-origin isolated page
can use the URLs; the signature is the only credential. The endpoint itself
counts and records nothing: `upload-urls` did before it signed.

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
  sign-in calls (`begin-browser-login`, `poll-browser-login`,
  `authenticate`, `create-local-profile`) need no sign-in, and every stream
  needs it, so a client opens its streams once signed in.
- **Signing in.** A connection's first call (after a ping) must sign it in:
  `authenticate`, or `create-local-profile` where local profiles are on
  (`client::signs_in`); any other closes it, as does none within
  `sign_in_timeout` (`ServerConfig`, `HANDSHAKE_TIMEOUT` by default), and so
  does `logout` (`Connection::closing`). Signing in through the browser
  follows RFC 8628's device flow over one-shot calls (`http::one_shot`):
  `POST .../call` carries a greeting and one frame, `begin-browser-login`,
  `poll-browser-login` or (local only) `create-local-profile`, and is answered
  and closed with no session, so nothing is held while a person approves it.
  `begin-browser-login` answers a code, a secret, the poll interval and the
  lifetime; a poll sooner than the interval fails `SlowDown` and grows it by
  `SLOW_DOWN`; an approval hands its token out to one poll. One-shot calls are
  limited per address (`MAX_CALLS_PER_ADDRESS` each `CALLS_WINDOW`).
  `every_call_states_who_may_make_it` makes every call of `be3:server`
  (`embed::METHODS`) and fails until a new one is made there too. A block the
  caller may not know exists is refused as `BlockNotFound`, like one that does
  not exist; `PermissionDenied` means it may know it exists.
- **The server's side** is a `be_wasm_ops::net::Peer` (`Side::Host`,
  `client.rs`). It answers calls in arrival order; work a task finishes later
  (the storage stream's) comes back to the loop as a `Late`. `WatchHub` sends each connection `Push`es, written to its open
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
  - Connections and HTTP sessions not signed in are counted apart
    (`address::Unsigned`), from a connection's first byte until it signs in
    (again after a logout): at most `MAX_UNSIGNED_CONNECTIONS` (a quarter of
    `MAX_CONNECTIONS`) and `MAX_UNSIGNED_HTTP_SESSIONS`, and
    `MAX_UNSIGNED_CONNECTIONS_PER_ADDRESS` and
    `MAX_UNSIGNED_HTTP_SESSIONS_PER_ADDRESS` for one address, counted by
    `unsigned_key` (an IPv6 /48). One past the overall limit lets go of the
    oldest rather than being refused (`Eviction`), so signed-in clients always
    have the rest of the room and a new client still gets in to sign in. A
    connection carrying HTTP posts counts as not signed in until a post is for
    a signed-in session.
  - Pending browser sign-ins and portal form tokens are kept in a
    `browser_auth::Bounded` table, indexed by age and by address: one address
    holds `PENDING_PER_ADDRESS` or `FORMS_PER_ADDRESS`, the server `LIMIT`,
    and another lets go of the oldest instead of being refused.
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
  `Rejected` with the head to merge against. It carries what changed since the
  expected head: an `ObjectChange` (objects added and removed; the server
  lists the commit's objects in `commit_objects` from the expected head's) and
  a `ReferenceChange` (references removed, and added at their index in the new
  list; a moved one is both). It writes the head, refcounts for its new
  objects and only the edges that changed: `block_edges.position` is spaced
  (`EDGE_SPACING`), so a reference put between two others takes a position
  between theirs and the rest stay. A first commit (no expected head) lists
  everything, replacing the references its block was made with.
- **Storage** is charged to each block's payer (`blocks.payer`): its parent's
  payer, or at the top its author (an agent's creator), counting each distinct
  object once per payer and every version a block keeps. A move re-charges the
  moved subtree to the payer of where it goes (`follow_payer`), the mover at
  the top; the trash changes nothing
  (`decisions/storage_is_charged_to_the_trees_owner.md`; `store/usage.rs`). An
  account may store `ServerConfig::account_quota` bytes, and an organization
  its settings' `storage_quota` bytes; an upload, publish or hold that would
  pass either is refused with `OverQuota` before anything is written, and
  nothing that frees space is ever refused. Each object `upload-urls` reserves
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
  ancestors). `BlockSummary` reports both; `account.storage-usage` reports the
  account's `held`, `pending` and `quota`, and `account.storage-changed` streams
  it: its value on opening, then again whenever the payer's `account_usage` row
  changes (`ServerStore::usage_changes`, a broadcast of payers each connection
  following its own filters; one that falls `USAGE_CHANGES_QUEUED` behind reads
  again). It needs an organization open, and a connection keeps one: opening
  another ends the first. An object no block holds any more stays while an
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
- **Watches and live sessions** are keyed by organization and block
  (`watch::Place`): a block id is the client's choice, so another organization
  can hold a block with the same one. Watching or joining needs `View`, and
  taking a session's lease needs `Edit` (a viewer's claim is refused, and the
  replica host ignores the refusal).

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
`publish`, `hold-objects` and `missing-objects` confirm the caller's own
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
`None < KnowExists < View < Edit < Manage`): administrators manage everything
but owner-only blocks, which they only know exist (guides/settings.md, Owner only);
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
drops any refused past that at once. A block's own listing (`read-block`),
which clients never unlist, does not count against
`MAX_LISTINGS_PER_CONNECTION`: it is bounded by the blocks the caller may know
of.

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

Each block's journal (guides/blocks/client.md, "The journal") is two tables,
`block_journals` (tag, generation, base) and `journal_entries`, written by
`be_server::journal` and deleted with the block. It is kept whether or not
anyone is in the session, so a crashed owner's edits wait for the next one. An
`append` is kept only from the current owner of its generation (it is relayed
either way, as a relay is), refused when the journal is another tag's until
the owner rebases, and bounded by `MAX_JOURNAL_BYTES`; `read-contents` answers
journals while they fit in a quarter of a frame.

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

Remote accounts register and sign in through the `/auth` portal, which
verifies the email before creating the account; the protocol has the browser
sign-in's one-shot `begin-browser-login` and `poll-browser-login` ("Signing
in" above), token `Authenticate`, header authentication and `Logout`. Registering needs an
invite code the operator made (`store/invite_codes.rs`), checked with every
other cheap check before the password is hashed and spent when the verified
account is made. Password hashing and checks (`store/passwords.rs`) run off the
database lock, a few at a time, behind a bounded queue with a small share per
address (an IPv6 address counts by its /64); past it the portal answers busy.
Failed checks lock out an (email, address) pair after five and an address
after twenty, doubling up to an hour; one failure is forgotten per quiet
`FAILURE_DECAY`. An email's own count, from every address, only slows it: past
`FREE_FAILURES_PER_EMAIL` each failure holds it for `EMAIL_SLOWDOWN`, so strangers
together get one guess per slowdown, and never applies to a signed-in browser
or to an address remembered for that email (one that signed in, or finished a
registration or reset from its mail), so strangers can't stop the owner
signing in. Failure records and remembered addresses are bounded. Mail links
are capped per email and purpose, per address, and per invite code for
registrations; the global cap covers only registration and email-change links,
which need an invite code or a signed-in account, so anonymous traffic can't
reach it (`store/portal.rs`). Resets need no global cap: at most three per
account. A reset for an email with no account does the same work, counts
against the address and a separate cap of its own, and sends nothing. `Be3-Gateway-Authorization` is left to a gateway in front.

An administrator `Invite`s an email under an id the inviting client picks;
only an account with that email can read or accept it, once, and the id
reaches the invitee only in the link (guides/blocks/keys.md, "Identity, new phrases and devices").
`CreateAgent` makes a passwordless editor account with a token
(guides/tracker.md), at most `MAX_AGENTS_PER_CREATOR` per creator; only its
creator lists, rotates the token of (`RotateAgentToken`, which revokes every
older one) or removes it. One `bound-to` a block is that block's agent
(guides/tracker.md, A block's agent): `Identity::bound` limits what it
reaches, `client::bound_may` what it may call, and `WatchHub::go_live` keeps
one connection of it open. Accepting an
invitation, or creating an agent, makes the two accounts contacts.

### Notifications

Push notifications for members who are not connected (`notifications.wit`,
`notifications.rs`, `push.rs`; plans/notifications.md for what is left). The
server never reads one: the text is sealed under the block's data key with the
block and the time it was sent (`Peer::notify`, `be_block_client::Notification`).
What it does learn: the block, the sender, when it was sent, whom it mentions,
who was not connected, and from each endpoint which push service each device
uses. Pushes are padded to one length (`push::PADDED_BYTES`), so not even the
text's length reaches the push service.

- **Endpoints.** A device registers one per account and organization
  (`register-endpoint`): Web Push (RFC 8030) at an `https://` URL on port 443,
  or an FCM registration token, with the device's RFC 8291 keys. At most
  `MAX_PUSH_ENDPOINTS`, the oldest going first; a member's go when they leave
  the organization. A Web Push URL may only resolve to a public unicast address
  (`push::is_public`: no private, shared or reserved IPv4, and IPv6 only in
  `2000::/3` outside 6to4, Teredo and the IETF's `2001::/23`) and no redirect
  is followed, so a client cannot have the server post into its own network;
  `PushConfig::allow_local_endpoints` lifts both for tests. Any public host is
  taken: allowing only known push services was left as a hosting decision. An
  endpoint is dropped when its service says it is gone (Web Push 404 or 410;
  FCM only `UNREGISTERED`, since a wrong project also answers 404), or after
  `MAX_FAILURES` failures in a row over at least `FAILING_FOR`; 429 and 503
  count as neither.
- **Modes.** Each account's mode per block (`notify_modes`: `all` keeps no row,
  `mentions`, `off`), deleted with the block, the member or the organization.
  `notify-modes` lists an account's; a device reads them when it connects and
  again on `notify-modes-changed`, which `set-notify-mode` announces to the
  account's other connections (be-host-stack `Shared::notify_modes`, published
  to apps as `NotifyModesWatch`).
- **`notify`** (Edit; at most `MAX_NOTIFICATION_BYTES` and `MAX_MENTIONS`,
  `MAX_NOTIFICATIONS_PER_MINUTE` per account and `MAX_NOTIFICATIONS_PER_BLOCK`
  per account and block) goes to every viewer of the block
  (`BlockGraph::viewers`) but the sender whose mode lets it through and who has
  no connection to the organization open (`WatchHub::connected`), with one
  query for the organization's endpoints. Each push carries a `PushMessage`
  (organization, block, the sender's account, which the server vouches for,
  and the sealed bytes), encrypted to the device (be-lib-web-push, which
  devices open with too). It goes at high urgency only to whom it mentions.
- **Sending.** Pushes queue per recipient account (`Pusher::queue`, at most
  `MAX_QUEUED_PER_ACCOUNT`, and `MAX_QUEUED` over the server) and go one at a
  time in order, its devices in the order they registered, so an endpoint that
  never answers delays only its own account; tests rely on that order to show
  a push did not go. A push about a block that already waits for a device
  replaces it. Each send holds a blocking thread, at most `SENDERS` at once
  over the server, so services that never answer cannot take the pool the
  database shares.
- **VAPID** (RFC 8292): the server makes a P-256 key on first start and keeps
  it in `push_key`; `push-key` answers its public half, which a browser
  subscribes with. Each Web Push request carries a token for the endpoint's
  origin, signed with it, whose subject is `--auth-origin` or the public URL
  (warned about at start unless `https://`).
- **FCM** (HTTP v1): with `--fcm-credentials FILE` (a service account's JSON
  key, `push::FcmConfig`), the server signs an RS256 assertion for an access
  token (kept until a minute before it expires) and sends a data message whose
  `data.push` is the same encrypted bytes, base64url, so a device decrypts
  both alike; Google sees only ciphertext. `offers-fcm` tells a device whether
  to register through it. Tests run fake push services and a fake FCM and
  token endpoint (`TestPushService`).

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
