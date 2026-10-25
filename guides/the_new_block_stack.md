# The block stack

Every block in the app lives in the `be-*` crates: `be-store`, `be-commit`,
`be-session`, `be-graph`, `be-protocol`, `be-model`, `be-block`, `be-vcs`,
`be-client` and `be-server`. The app is a peer of its own (`crates/block-app/src/be.rs` and
`be/`), the server is `be-server`, and every editor keeps its content here.

To add a block type, read `guides/adding_a_block.md` first; it is the short
version of "Adding a content type" below. This guide is for work on the stack
itself and on how the app hosts it.

## Why the stack looks like this

It replaced an older block system whose `Block` trait fused five separable
things into one type: the durable representation, the live-edit operation, undo
history, the reference graph, and child manipulation. `const CRDT: bool` then
made a single choice that governed live merge, offline merge and the wire
protocol together. Most of what went wrong followed from that coupling, and
each of these is something not to bring back.

- **The operation log never compacted.** Reading a block replayed every
  operation it had ever received, and it could not be fixed in place:
  operations were ciphertext to the server, and compaction needs understanding.
  An append-only operation log and end-to-end encryption are incompatible.
- **References cost an organization per keystroke.** The client replayed pending
  operations to diff the full reference set, and the server rewrote every
  reference row in the organization on every update. Under a CRDT the delta was
  also wrong, because it was computed against an optimistic local state rather
  than the converged one.
- **There was no local persistence.** Unsent work lived in memory, so closing
  the process lost it.
- **Bulk data was in band.** An image was base64'd into JSON, wrapped in a
  snapshot, and shipped whole on every replace.
- **The layering was inverted.** Version control was a content-addressed blob
  store with commits and branches, built as block types on top of the block
  system. The thing that should be the substrate was a guest.

## Goals

1. Durable state is bytes. A block's persistent form is a byte string, stored as
   an ordered list of chunks. Backups are a commit chain over those bytes.
2. Live editing converges through a session owner, not through a CRDT. A block
   type may still be a CRDT where that is genuinely better, but nothing depends
   on it.
3. Offline divergence resolves by three-way merge over the persistent bytes, so
   a wholesale rewrite conflicts visibly instead of interleaving.
4. Partial reads. A ten gigabyte video streams; it is not loaded to be shown.
5. The server never sees content. It holds objects, refs, a graph and accounts,
   and understands only the last three.
6. Reclamation is reference counting, not tracing.

## The layers

```
be-store     objects, chunking, encryption        client + server
be-commit    commit DAG, retention, merge         client + server
be-session   ownership lease, sequencer, resume   client + server
be-graph     parents, edges, access, refcounts    server
be-protocol  the wire format                      client + server
be-model     objects with ids, merge, undo, derive client
be-block     content traits and content types     client
be-vcs       snapshot trees, diff, tree merge      client
be-client    a peer: local store, blocks, relay   client
be-sync      a folder <-> a directory on disk      client (native)
be-server    the always-online peer               server
```

The server is one more peer that never decrypts. That is why most crates are
shared: a client is a peer with a local store and a UI, a server is a peer with
a big store, no UI, and the session registry.

### be-store

Content-addressed, encrypted object storage.

- `Hash` is the SHA-256 of the **ciphertext**, so a server can verify what it
  holds without being able to read it.
- `chunker::split` is a gear-hash content-defined chunker. An edit in the middle
  of a large file re-uploads one chunk, and the chunks either side resynchronise.
- `Vault` seals with a key it is named by id from its `Keyring`, and opens by
  the id the sealed bytes carry (see Keys). Sealing derives its nonce from the
  key and the plaintext. That is deliberate: it makes identical content
  deduplicate under one key, and share nothing across keys. Do not replace it
  with a random nonce without replacing the deduplication story too.
- `Manifest` is `{content_type, length, chunks}`. `Vault::read_range` and
  `chunks_in_range` are the streaming primitives.
- `ObjectStore` is `MemoryStore` or `FileStore`. It is synchronous on purpose;
  network fetching is explicit (see `Peer::fetch`), so nothing blocks inside it.
  `FileStore::put` writes atomically (a temporary file renamed into place) but
  does not wait for the disk; `sync` makes everything put so far durable at
  once (one `syncfs` on Linux and Android). Call it before telling anyone else
  an object exists: `Peer::push` syncs before it uploads, and the server syncs
  a `PutObjects` before it answers. A torn object left by a crash fails its
  hash on reading and is dropped.
- `Vault::put_value` stores a value (a commit, a version-control tree) as a
  CBOR map by field name (`encode_value`), so a later build can add a field to
  one, with `#[serde(default)]`, without a migration. Commits are encrypted, so
  only a client could ever rewrite one, and rewriting one changes its hash and
  every id that names it: an encrypted format is read as it was written,
  forever. A hash is a CBOR byte string.

### be-commit

- `Commit` is `{parents, manifest, author, time, kind, references, metadata}`,
  stored as an encrypted object whose hash is its `CommitId`. Several parents
  means a merge. `Publish` also tells the server the parents in plaintext, so
  the server knows the history graph of every block without reading a commit.
- `references` is the edge set derived from the **sealed** state, recorded once
  per commit. This is what makes the reference graph correct: it always describes
  content that was actually stored.
- `CommitStore` walks the DAG and tolerates pruned history: a missing commit
  truncates a chain instead of breaking it, so `first_parent_chain`, `ancestry`
  and `common_ancestor` all keep working after retention runs.
- `retention::plan` thins history. It pins bookmarks, keeps everything inside
  `keep_all_within`, and then thins by widening age buckets, preferring within
  each bucket the commit with the longest **quiet interval** after it. A state
  someone left alone for an hour outranks the keystroke bursts around it.
- `merge` is diff3. `merge_slices` walks the base indices that both sides left
  equal; the regions between those anchors take whichever side changed, or
  conflict when both did. `merge_lines`, `merge_map` and `render_conflicts` are
  built on it.

### be-session

- `Lease` is ownership. The first participant owns the session; a claim succeeds
  only against the generation the claimant last saw, and only when the lease has
  expired or the owner is gone. Generation numbers stop a stale claim from
  stomping a newer owner. A heartbeat (`ReportClean`) renews the lease: the
  owner sends one after every seal and, while it holds the session, every
  `HEARTBEAT_INTERVAL` (half of `LEASE_MILLISECONDS`) on its replica's own
  timer (`wake_at`), with `clean_at` set only while it has nothing unsealed. Its length is `ServerConfig::lease`; tests that claim pass zero
  (block-app's `Harness::lapsing`, `Lease::lasting(0)` in the replica's `Sim`)
  so a claim never waits on the clock.
- `clean_at` is the commit the owner last reported its state durable at. It
  outlives the session going empty (see `sessions::forgettable`), so a peer
  taking over an abandoned session can tell whether anything was unpublished.
- `resume` decides whether a returning peer merges, and the answer comes from
  history, not from having been away: an ancestor head fast-forwards, a
  descendant head publishes, and only genuine divergence merges.
- `Sequencer` and `Follower` are the two ends of the ordering. Operations carry
  an `OpId`, so work a peer submitted before losing its connection is
  deduplicated on the way back in rather than applied twice.

### be-graph

Liveness is hierarchical. A block is live when its parent chain reaches
`BlockParent::Root` and every link on the way holds: a parent holds a child
only while the parent's references list it (`BlockGraph::holds`). A block whose
parent is `Detached`, or whose parent no longer lists it, is in the trash
**together with its subtree**, even when something else still references it,
and it keeps its parent. So deleting a block is an edit to its parent's content
that drops it (`ChildChange::Delete`), and listing it again, by undo or by
Restore, brings it back. A reference from anything but the parent is a link,
for navigation and access, and keeps nothing alive. `BlockGraph::deleted_roots`
are the blocks whose own link does not hold; `deleted_root` finds the one a
block is deleted under.

Each block still has one parent, so liveness is a walk up one chain, and that is
what makes reclamation reference counting rather than tracing: a block holds
its commits, a commit holds its chunk hashes, and `ObjectRefs` deletes an
object when the last holder releases it. There is no cycle to trace and no mark
and sweep. Do not reintroduce one.

Every content type therefore lists the blocks it is the parent of among its
references, and whatever makes a child under a parent also adds it to the
parent's content. Until that edit is saved the child shows in the trash for
the parent's other viewers; the app hides the window on its own side by
treating the block as being placed (`Graph::placing`) until the parent lists it.

### be-server

SQLite for metadata, a `FileStore` for objects, and a websocket carrying
postcard frames. Where a websocket cannot get through (a proxy that refuses the
upgrade), `POST .../http` carries the same frames (`http.rs`): the first body is
the greeting, whose answer names a session in a `Be3-Session` header; each
later post with that header carries client frames, answered in order with the
frames the server has for the client then; a body is frames each after a
four-byte big-endian length. The client sends whatever is queued in one post,
and `Peer::fetch` asks for every missing object at once. A
session is a connection like a websocket's, kept until it is idle for the same
time, so notifications wait for the next post.

- The database's schema is a list of `CREATE` statements (`schema.rs`), and
  `sqlite_schema::ensure` checks a database against it on open: it adds a
  missing table or index and a missing column that has a default, and refuses
  anything else (a column gone, retyped or unknown), so a schema only grows.
  Values that may grow (a role, an access level, a parent kind) have no `CHECK`,
  since SQLite rebuilds a table to change one. The app's own `app.sqlite3`
  is checked the same way, and recreated (signing the device out) when it
  does not match.
- A connection starts with a greeting outside the protocol: each side sends
  `be_protocol::hello` with its build id (`build_id::current()`, read from the
  `build-info.json` file beside the executable, or `--build-info-file`), and the server
  closes the connection when the ids differ (`ClientError::Incompatible` on the
  client). A program with no id is an error, not a wildcard: the client fails
  with `ClientError::NoBuild`, the server does not start, and a test installs
  one with `build_id::install`. Server and clients are upgraded
  together from one commit, so the protocol itself is postcard and never
  versioned.
- A publish is a compare-and-swap on the head. A publish against a stale head
  comes back `Rejected` with the head to merge against, rather than being
  silently ordered after it.
- A publish writes one head update, the handful of edge rows the commit
  declares, and refcounts for chunks it actually introduced. It does not rewrite
  the organization.
- The tenant is an organization (`organizations`, everything keyed by
  `organization_id`): one block graph, its members and its administrators.
  Its `kind` is `company`, made with `CreateOrganization` by an administrator,
  or `personal`, the server's one organization everyone may share, which
  `ServerStore::personal_organization` makes on demand; registering joins it
  as an editor, and nobody administers it.
- Access is computed per member (`Visibility` in `blocks.rs`), as
  `None < KnowExists < View < Edit < Manage`. An organization administrator
  manages everything. Anyone else gets the best grant on the block or an
  ancestor (a grant that can view is inherited down, and a lower grant never
  reduces it), and `Manage` on what they authored when its parent is `Root` or
  `Detached` or they can view its parent, so a member taken off a workspace
  loses what they wrote inside it. A settings block (`be_graph::SETTINGS_TYPE`)
  whose parent is a block is the exception: it is `Manage` to whoever manages
  its parent, at most `View` to whoever can view the parent, and its own
  grants never raise that, so changing, adding or moving in a block's
  settings needs `Manage` on the block. The kinds an editor changes
  (`be_graph::EDIT_KINDS`: the prefix registry and tag definitions) are not in
  the settings block but are children of the block itself, so they inherit
  `Edit` like any child, though a grant on one never raises it above its
  parent's. Below a settings block or such a kind, every block has exactly
  the access of its parent: neither authorship nor a grant raises it. Who
  reads a kind is a manager's choice; the editor who made an edit kind may
  only open it to anyone inside once, as it is made (its default).
  Moving a block (`SetParent`) needs `Edit` on it and on the new parent, and
  `Manage` on it to move a workspace (a root block) or to move it into another
  workspace (`BlockGraph::move_refusal`); without `Manage` a move is also
  refused if it would raise the mover's own access to the block (into a folder
  they manage, say). An author manages what they made, so moving one's own
  block anywhere one may edit is fine.
  A block they can view references gives them `KnowExists` on it, and on
  nothing that block references in turn. Of a block they may only know exists
  they are told its id, content type and access and nothing else
  (`BlockSummary::no_access`: no author, parent, head, version, references,
  metadata, key or scope), and are served none of its keys or objects. Ancestors
  are not shown: a block someone can see whose parent they cannot is an entry
  point, which the app lists under Shared with me. `SetAccess` grants, and only
  someone who manages a block may grant on it or list its access; a grant above `None` must also
  name a member of the organization who is a contact of the granter (see
  Contacts). Anyone may revoke their own grant (leaving a block). `ListAccess`
  lists the block's author, grantees and viewers and the caller's contacts who
  are members, never the rest of the membership, and marks a grantee still
  `waiting` for the block's scope key. An administrator lists the members
  (`ListMembers`), changes a role (`SetRole`) and removes a member
  (`RemoveMember`, which revokes their grants and drops their wrapped keys);
  the organization always keeps one administrator, and the affected account's
  connections to it are closed (`WatchHub::drop_member`), so they reconnect
  with what they now are.
- Nothing that runs per request or per publish maps the whole graph.
  `BlockGraph` indexes children by parent and blocks by the accounts that
  authored them or hold a grant on them (their entry points).
  `BlockGraph::inherited` walks up from one block, which answers every edit
  and view check; `visible` walks down from an account's entry points to list
  what it knows of; `audience` names the accounts that may know a block: the
  authors and viewers of its parent chain, its own grants and the viewers of
  what references it. A move or a grant changes what is below the block only
  for accounts in its audience before or after, so nothing walks the subtree
  to find them.
- Graph announcements are per member too: when a block changes, each connected
  client whose account is in the block's audience before or after the change
  (and every administrator) is sent the `BlockChanged` its own access lets it
  see, or a `BlockRemoved` when it no longer does. The blocks a changed block
  references, and those a publish stopped referencing, are announced with it,
  since who may know they exist changed too; an id no block has (one linked
  before it is made) is never announced. `WatchHub` indexes clients
  by organization and account, so this never scans every connection.
- A connection hears only of what it listed. `ListChildren`, `ListShared`,
  `ListBackrefs`, `ListReferences` and `ReadBlock` each register a `Listing` with `WatchHub`
  before reading (so nothing changed during the read is missed), and the hub
  remembers each block a connection has been told of. A change is sent when a
  listing matches the block as it now is (its parent, its own id, an entry point
  under `Shared`, a reference to a backrefs target, being referenced by a
  block whose references are listed, as the connection last heard of that
  block) or when the connection was
  told of it before, so a block moving out of a listed folder is still
  announced, and then forgotten. `Unlist` drops a listing and the blocks only it
  matched; a refused listing stays until then, so a block read before it is
  made is announced once it is.
- Every `BlockSummary` and `BlockRemoved` carries the `revision` it was read
  at: `ServerStore::with_graph` advances one counter on each call, under the
  lock every graph read and change takes, so a larger revision is always a
  later view. Announcements and replies can reach a client out of order (a
  reply overtakes the announcements queued before it, and two requests'
  announcements interleave), and the revision is how the client puts them back
  in order. The counter restarts with the server, which a client sees as a
  reconnect and a reload.
- Accounts live here. Remote password authentication and verified registration use
  the `/auth` portal. The protocol uses browser sign-in handoff, `Authenticate`
  with a token, header authentication, and `Logout`; organizations are created
  through it, and an administrator can `Invite` an email under an id the
  inviting client picks. Accepting an invitation makes inviter and invitee
  contacts. Only an account with that email can read the
  invitation (`GetInvitation`) or join with it (`AcceptInvitation`), once;
  inviting the same email again replaces it. There is no list of an account's
  invitations: the id only reaches the invitee in the link (see Keys).
  Remote accounts register and sign in through the server’s `/auth` portal;
  registration verifies email ownership before creating the account. Password
  checks run off the database lock, a few at a time, and an email with five
  failed sign-ins in a row is locked out for a time that doubles with each
  further failure.
  The server never reads `Be3-Gateway-Authorization`, which is left to a gateway in front
  of it. A member's `CreateAgent` makes an agent in either organization kind: an
  editor account with no password, with the recovery public key the caller made
  for it, and a token, answered once (guides/tracker.md); the agent and its
  creator become contacts. Only the creator can list or remove that agent.
- An object is served only to a member who can view a block that holds it, by
  a commit (`commit_objects`) or by `HoldObjects`; anything else reads as
  missing. A merge commit holds its extra parents' objects, so a `stash` that
  was never a head stays readable.
- The graph is cached per organization in memory and dropped on any error so it
  reloads from the database rather than drifting.

### be-block

A block's content is a set of traits a type opts into.

```rust
trait BlockContent {                    // durable form and the edges it declares
    const CONTENT_TYPE: Uuid;
    const FROZEN: bool = false;
    fn schema() -> String { String::new() }
    fn encode(&self) -> Vec<u8>;
    fn decode(bytes: &[u8]) -> Result<Self, ContentError>;
    fn references(&self) -> Vec<Uuid> { Vec::new() }
    fn references_in(&self, organization: Uuid) -> Vec<Uuid> { self.references() }
    fn name(&self) -> Option<String> { None }
}
trait LiveEdit: BlockContent {          // optional: what a session carries
    type Op;
    fn apply(&mut self, operation: &Self::Op);
    fn rebase(operation: Self::Op, onto: &[Self::Op]) -> Option<Self::Op>;
}
trait Merge: BlockContent {             // required for offline editing
    fn merge3(base: &Self, ours: &Self, theirs: &Self) -> MergeResult<Self>;
}
trait Undo: LiveEdit {                  // optional: client-local undo
    type Step;
    fn step(&self, operation: &Self::Op) -> Option<Self::Step>;
    fn absorb(previous: &mut Self::Step, next: Self::Step) -> Result<(), Self::Step>;
    fn revert(&self, step: &Self::Step) -> Vec<Self::Op>;
    fn reapply(&self, step: &Self::Step) -> Vec<Self::Op>;
}
trait Streamed: BlockContent {          // optional: header plus payload
    type Header;
    fn header(&self) -> Self::Header;
    fn payload(&self) -> &[u8];
    fn from_parts(header: Self::Header, payload: Vec<u8>) -> Self;
}
```

History and child manipulation are not part of the durable form. Undo is
client-local and against a sequencer it emits an inverse operation rather than
rewinding state; a child change is an ordinary operation that
`LiveEdit::child_operations` answers for. `Undo::step` is taken from the state before an
operation, and `revert` and `reapply` turn a step into operations against the
state as it is *now*, so an undo leaves alone whatever someone else changed
since: the calendar undoes a rename without moving an event another peer
rescheduled.

`Streamed` types encode as `[u32 header length][header][payload]`, which is what
lets a reader take the header and then a byte range. Every `Blob` (below) is
streamed.

`references_in` is `references` for one organization, and it is what the peer
calls when it records a commit's references (`Root::references_in` for a
document). Text uses it: a block URL names its organization, and a URL pasted from
another organization is a link, not a reference.
The URL format itself lives in `be_block::block_url`: a block URL is
`<base>#block/<organization>/<block>`, where the base is the URL of the server the
organization is on (the `LinkBase` host value), sub-path and all, or nothing for
the device's own server. Text stores these bytes, so the format is read
forever: a reference is found by its fragment alone, whatever host comes before
it, and the link it belongs to starts at the `http://` or `https://` directly
before the `#`.

### be-client

`Peer` is a local object store plus a connection. `transport/` is what makes
that connection portable: a `Writer` and a `Reader` per platform - tokio and
tokio-tungstenite natively, the browser's own `WebSocket` in `web.rs` - plus the
`spawn` each one's executor wants. Natively, an `http(s)://` URL (`http_url`)
takes the server's HTTP posts instead of a websocket (`transport/native/http.rs`),
and both go through `HTTPS_PROXY`. `be_client::authorize` registers an
`Be3-Gateway-Authorization` header for a server's origin, which every native connection to
it sends; a browser cannot add one to a websocket. `Connection` above it is the same code
everywhere, and it waits on its command channel and its socket rather than
polling either.

`Connection` also decides when the server is gone, because a socket can stay
open with nothing behind it (a laptop that slept, a Wi-Fi hop, a proxy that
stopped forwarding) and the transport never says so:
- Connecting, the socket plus the greeting, gives up after `CONNECT_TIMEOUT`
  (15 s), and a request once the server has sent nothing for
  `REQUEST_TIMEOUT` (60 s) since it was written, with `ClientError::TimedOut`.
  The server answers in order, so a request queued behind slow ones is not
  timed out while their answers keep coming. `ClientError::is_unreachable`
  covers it and `Disconnected` both.
- After `PING_INTERVAL` (15 s) with no frame of any kind from the server, it
  sends `ClientMessage::Ping`. A browser cannot send websocket pings, so this
  is a protocol message, and any frame proves the server is alive. The server
  reads frames in a task of its own (`serve_socket`) and answers a `Ping` (and
  a websocket ping) at once, ahead of the requests queued for its answering
  loop, so a slow request does not make the client think the server is gone.
  `PONG_WAIT` (20 s) later with still nothing heard, the connection closes
  itself, and everything waiting on it sees a disconnect. Silence and request
  deadlines are only judged when the timer fires and no frame is waiting to be
  read, so a client whose thread was busy reads what arrived before deciding,
  and time spent writing a large frame does not count as silence.
- Each deadline is read from a `be_client::Clock`, passed to
  `Connection::connect`: `Clock::real()` everywhere but tests, which pass
  `Clock::manual()` and `advance` it, so none of this needs sleeping in a test.
  `crates/fault-relay` is the other half of those tests: a TCP relay between a
  client and a server that can `stall` its connections (read, count what it
  heard and forward nothing, a half-open socket), `cut` them, or `hold_new`
  ones it accepts without ever reaching the server.

- `open` reads the head commit and its chunks with one `ReadContents` and decodes.
- `save` chunks the content and publishes against the head it read, carrying in
  the `Publish` the commit and the chunks that head lacks, so a save is one round
  trip. A merge, or new objects over half a frame, are uploaded first instead:
  only what `MissingObjects` says the server lacks, in `PutObjects` of up to
  2 MiB each.
- There is no batch request. The server answers a connection's requests in
  order and both transports send whatever is queued together (the HTTP one in
  one post), so independent requests go out without awaiting each other; only a
  chain where each request needs the last one's answer is worth a request of its
  own.
- `fetch` verifies that bytes the server returns hash to the hash that was asked
  for. The server is not trusted with content, so do not drop this check.
- `fetch_history` walks a commit's parents into the local store. Anything that
  asks history a question (`resume`, `common_ancestor`) needs this first, or an
  unfetched ancestor looks like a divergence.
- `stream_header` and `stream_range` read a `Streamed` type without the payload.

A block being edited is in a session, which each device holds in its type
module's replica (`block-type-guest`, guides/type_modules.md); the peer only
relays, seals and stores for it. A relay goes on the connection in order without
waiting for the server's answer to the one before (`Peer::relay` returns it, and
be-types-host waits for it apart), so a pen stroke's thousands of operations cost no more
round trips before the seal than one does. The owner sequences and broadcasts; a follower
applies its own edit immediately, rebases it against operations that arrive in
between, and reconciles against the owner's echo. A replica keeps `confirmed`
(server-ordered) and `visible` (`confirmed` plus pending). A seal writes a
commit, heartbeats `clean_at`, and tells the followers with
`SessionMessage::Sealed` which commit now holds which sequence number. A
reconcile runs the resume decision and merges when it has to.

A follower's `head` is only right because of `Sealed`, and ownership depends on
it being right. A follower that takes over carries on from the head the last
owner sealed: it keeps the old sequence numbering so the other followers keep
applying what it accepts, it accepts the operations it had pending as its own,
and it owes a seal for whatever the old owner accepted after its last one.
Get the head wrong and the new owner's first save is refused, and the resume
decision reads its unsaved work as something to fast-forward over. A reconcile
merges unsaved work into anything it fast-forwards or merges onto, and when
it changes the owner's content outside the operation stream, the next `Sealed`
tells the followers to reload the head. Only an owner ever seals or reconciles:
a follower's edits are the owner's to save.

Ownership also moves while the old owner is still there: a host's
`Event::Claim` takes a lapsed lease (a terminal host takes the block from the
viewer that opened it first; a replica only claims on its own when the owner
has left). The old owner learns of it from the next `Lease` and becomes a
follower that has applied everything it sequenced, so its status stops saying
owner and it stops sealing. What it accepted before it heard reaches the new
owner as `Accepted` from a non-owner: `Sequencer::inherit` keeps it when it
continues the new owner's numbering, and otherwise the new owner applies it,
owes a seal and makes that seal reload every follower, which is what reconciles
the two numberings. A follower is catching up from loading until the owner's
`Snapshot` arrives, and re-sends its `Catchup` to each new owner meanwhile; a
replica ignores `Event::Claim` while it is catching up, because an owner that
never heard the snapshot would number from where it joined and lose the old
owner's unsaved operations. A follower promoted while catching up, by the owner
leaving, makes its first seal a reload for the same reason.

A merge that conflicts first writes the owner's unsaved side as a commit of its
own (`Command::Stash`), so the merge commit has both sides as parents and neither
is lost, and reports a `be_types_host::Conflict` to its host. block-app keeps
it per block (`be::conflicts`) and shows a notice on the block whose "See both
versions" previews the two sides as revision previews; keeping the merge or
restoring one side clears it. A session message
or operation this version cannot decode marks the session incompatible. On a
follower that also marks it diverged, because its copy may now be wrong, and a
diverged session refuses to edit or seal.

## Adding a content type

Describe the content as structs and derive `be_model::Model`; do not write a
merge, a rebase or an undo.

```rust
#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Calendar {
    pub events: List<CalendarEvent>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct CalendarEvent {
    pub title: String,
    pub start: i64,
    pub end: i64,
}

impl Root for Calendar {
    const CONTENT_TYPE: Uuid = ...;
}

pub type CalendarContent = Document<Calendar>;
```

A field is one of ten things. A `Count` is a counter whose concurrent changes
add up. A `Grid<T>` is a dense block of fixed-size cells addressed by
coordinates, for pixel data: its bounds are part of its value, so a resize moves
the bounds and keeps every cell at its coordinates, `paint` sets cells and
`reshape` sets the bounds. A `List<T>` holds objects of a `Model` type `T`. A `Map<K, V>` holds
values by key, each key its own register: two people setting different keys
both keep theirs, which is how a database row holds a cell per schema field.
`Latest<T>` and `LatestMap<K, V>` are last-write-wins: every write carries a
`Stamp` (a time and the writing client), a write only replaces an older stamp,
and a merge keeps the later stamp per key, so they never conflict and never
undo. `next(time, origin)` stamps a write past the one it replaces, so a client
with a slow clock still overwrites what it last read. View state uses them.
A `Text` is bytes edited in place: it saves only its bytes, and a live session
gives every byte a position (`Sequence`, in the `sequence` crate) that edits anchor to,
so concurrent typing and deleting never need rebasing. Positions and tombstones
are session state (positions only: the receiver fills the visible bytes in from
its own copy, and an undelete carries the bytes it brings back), and an offline
merge is a line diff3. The text block is `Document<TextBlock>`, its body a
`Text` (see `plans/text.md`). A `RichText` is a `Text` with formatting, the
marks beui's rich text editor makes (`sequence::marks`, re-exported as
`be_model::marks` and by `text_editor_core`): each mark is a key and value
(bold, a link, a heading level) over a range, inline or over whole lines,
anchored to positions, so its edits are `Change::Text` for the bytes and
`Change::Mark` and `Change::Unmark` for the formatting. A mark has an id
(a clock and the client), the later id wins where marks overlap, and
concurrent formatting and typing converge like typing does. It saves its
bytes and its formatting as byte ranges, resolved so no two ranges of one
key overlap, and reloads them as fresh marks; the session state carries the
live marks with the positions. Undoing a format removes that mark only. An
offline merge is a line diff3 where a line is its bytes and its formatting
together, so formatting a line on one side and typing in it on the other
conflicts and is rendered with the same markers as text.
A `Series<T>` is a list of register values on the same `Sequence`, for data
that grows at its end, like a stroke's points: `update` diffs what is wanted
against what is held and sends only what changed after the common start
(`Change::Series`), so appending costs the new items, not the whole list. It
saves as an array of its values, the same as a `Vec<T>` register, so a
register that becomes a series keeps reading what was saved; an offline merge
keeps both sides' appends and otherwise picks one side as a register does.
An object projection re-reads with `Model::reread`, which hands a `Series`
what it read last: it decodes only past the fragments that are unchanged.

Client `0` (`sequence::LOADED`) belongs to the initial buffer; edits cannot
allocate positions under it. The text editor anchors each boundary before one
byte, including deleted bytes through their tombstones. Its end anchor is
`Pos { client: 0, offset: sequence.next_offset(0) }`, the position just past the
initial buffer. This virtual sentinel resolves to the current visible length
without adding a byte to the sequence or its saved text. `Position` wraps only
that `Pos`; construct the end through the document rather than a global constant.
IME composition tracks its last byte separately so text inserted after that
byte stays outside the composition.

Anything else that is `Serialize + DeserializeOwned + Clone + PartialEq +
Default` is a register: it is set as a whole, and setting it on both sides of an
offline merge is a conflict. `Root` names the content type and, optionally, the
block's name and the blocks it references.

### The stored form

`Document::to_bytes` writes CBOR (`ciborium`) that names everything and holds
nothing a live session needs: no positions, tombstones or op history. It is
`{"format": N, "root": object}`, and an object is a map of its properties by
field name plus `$kind` (the struct's kind: `#[model(kind = "canvas.entity")]`,
or its name in snake case) and, for an object in a `List`, `$id` (its `ObjectId`
as a UUID, tag 37, so offline merges match objects across commits; a repeated id
is given a fresh one on load). A register is its serde form in CBOR (structs as
maps by field name, enum variants by name), a `Text` is bytes (any bytes, not
only UTF-8), a `List` is an array of objects, and a `Map` an array of
`[key, value]` pairs. The same CBOR is what registers hold in memory and what
`set` sends, so a value read back is byte-for-byte what was saved.

The format is meant to change without migrations:

- **Adding is free.** A new field, a new field inside a register's struct, a
  new enum variant or a new kind needs nothing: a missing property reads as its
  default. Give every struct a register holds `#[serde(default)]` (and every
  field of an enum's struct variant `#[serde(default)]`), or a document saved
  before a field was added loses the whole register;
  `Kind::registers_accept_missing_fields` lists the ones that would.
- **Unknown data is kept.** Properties a build does not know are kept on their
  object (`Object::kept`), merged key by key, and written back, so an older
  build never drops what a newer one wrote. A register whose bytes this build
  cannot decode reads as its default but keeps its bytes until it is set.
- **Renames.** `#[model(rename = "...")]` keeps a field's stored name when the
  Rust name changes, `#[model(alias = "old")]` reads an old name as well, and
  `#[model(from_old("old", convert))]` reads an old name through
  `fn convert(Old) -> New`, for a field whose meaning changed. Old names are
  read, never written.
- **Critical fields.** `#[critical]` on a field means an older build must not
  edit an object it cannot read fully. A critical field set to something other
  than its default is saved inside tag `CRITICAL`; a build that does not know
  the field locks that object (`Document::is_locked`): its fields, and lists,
  refuse edits, though it can still be moved or removed whole. Everything else
  in the document stays editable.
- **Reshapes.** A change that moves data between objects bumps the root's
  `#[model(format = N, migrate = path)]`; `migrate(old_format, &mut root)`
  rewrites the stored root before it is read. A build refuses a format newer
  than its own.
- **Block references.** A block id stored as `BlockRef` (or a `Uuid` field
  with `#[serde(with = "be_model::references::block_ref")]`, `::option` for an
  `Option`) is saved under tag `BLOCK_REF`, and `Document::block_refs` finds
  every one, including those inside properties this build does not know. The
  content's references are those plus `Root::references`.

Freezing a version: set `Root::FROZEN` (or `BlobKind::FROZEN`), which turns off
the stale-block handling below for the type. `be_model::schema::freeze(&document)` returns Rust source
holding the type's registry (`Kind::describe`: every kind, property, shape,
register default and old name) and the document's bytes. Keep it as a module
of the block's tests, and test it with `schema::check_frozen::<Root>(REGISTRY,
DOCUMENT)`, which fails when a frozen property is gone or retyped, a register
default changed, the document no longer loads, or anything in it is no longer
understood. guides/frozen_formats.md lists what is frozen and the end-to-end
snapshots of a server that hold it.

`be-model` stores a document as a table of objects with ids, not as a tree of
values, and every algorithm is written once against that table:

- **Identity.** Every object in a `List` has an `ObjectId` (the `id` of the
  `Item` the list reads back as), and edits name the object, not a position.
  An edit to an object that has since moved into another list still lands on
  it, and a `move_into` carries an object and everything under it from one list
  to another of the same kind (by the schema's kind name); a move into a list
  of another kind changes nothing, since saving would retype the object.
- **Edits.** The derive gives each field a typed constant, `CalendarEvent::TITLE`,
  that builds the changes an `Edit` is made of: `set` for a register, `add` for a
  `Count`, `put` for a key of a `Map`, `insert` and `move_into` for a `List`, and
  `Change::remove` for any object. A content type usually wraps these in helpers the editor calls, like
  `Calendar::update`, which only writes the fields that changed.
- **Live editing.** Edits address objects by id and anchor inserts to a sibling,
  so they mean the same thing whatever the sequencer put before them: there is
  nothing to rebase. A list is a `Sequence` of ids (`Items`) that remembers each
  object's slot in it. An insert, or a move into a list the object was never
  in, takes the next offset of the client that made it (`local_client`, carried
  in the change), so every replica gives it the same slot. Removing an object
  leaves a tombstone, an insert anchored after a removed or moved-away object
  lands where it was, and an object that comes back to a list reuses its slot.
  A move within a list keeps the slot; a move to another list leaves a tombstone
  behind. Tombstones and text positions live only
  in the session: they are never saved, the owner hands them to a joining
  follower in `Snapshot` and to reloading followers in `Sealed`
  (`LiveEdit::session_state`), and they go when the session ends. A `Snapshot` carries the state as of the
  owner's last seal, because the follower opens the sealed head and applies the
  operations since onto it; adopting state rebuilds a follower's view from
  `confirmed` and its pending edits rather than adopting into it. Anything two peers may
  create at once for the same purpose (a database row past the end, a canvas
  component for a schema, a logic game solution) takes an id derived from what
  it is for, so the second insert is refused and its edits land on the first.
- **Offline merge.** `Document::merge` matches objects by id across the three
  versions and merges each field on its own: registers take whichever side
  changed, counts add both sides, lists merge their order the way `merge_slices`
  merges lines once the items either side moved are taken out and put back after
  the sibling they followed on that side, and an object that moved on one side
  and was edited on the other keeps both. You delete what you saw: deleting an
  object (or map key) the other side edited deletes it and counts a conflict.
  What the other side moved or inserted into a removed container, which the
  deleting side never saw there, survives: a moved object goes back to the list
  it came from (or the first sibling container), a new one brings its container
  back, and either counts a conflict.
- **Undo.** `Document::step` records, for each change, the change that undoes it
  and the one that redoes it, both taken against the state before the edit. A
  register's undo is conditional (`Change::SetIf`, and `Change::PutIf` for a map
  key): it only puts the old value back if nobody has changed it since, which is how undo leaves other
  people's edits alone. A move's undo puts the object back after what preceded
  it, even if someone moved it since, and its redo moves it to where the move
  put it. A removed object is put back with everything under it, in its old
  slot. `Change::RemoveIf` removes
  an object only if it still holds what the remover expected, for cleanups like
  a database dropping a row it emptied. Consecutive sets of the same fields
  absorb into one step.
- **Grids.** A paint is a list of cells, each optionally conditional on what the
  cell held, so a paint's undo repaints only the cells nobody has painted since,
  and a burst of paints into one grid undoes as one stroke. An offline merge
  takes the bounds the way it takes a register and then merges each coordinate
  on its own, so a paint made during a resize lands where it was painted, and a
  crop's undo repaints what the crop dropped.

`Document<R>` implements `BlockContent`, `LiveEdit`, `Merge` and `Undo` in
`be-block` (`model.rs`), so a type built this way is registered with
`undo` in `be_block_types!` and has undo from the start. Almost every content type is
built this way: the counter, the checklist (the one first-party type already in
its plugin's types crate, `crates/editors/checklist/types`), the calendar, the browser
tab, the UI settings, the three database types, the presentation, the hotbar,
the deterministic game, the map, the video, the logic game, the logic grid,
compiled logic, the infinite canvas, pixel art and text. Implementing the
traits by hand remains possible for content that does not fit. The browser tab shows a register holding an
`Option<ObjectId>`: its current page is an object in its history, not an index,
so a push and a navigation made at the same time still agree on which page is
current. The video shows what identity buys a tree: a clip attached to another
is an object in that clip's list, so reattaching it is a move, a move that
would make a cycle is refused by the model, and removing a clip takes what is
attached to it. Its editor still speaks in `VideoOperation`s, which
`VideoProject::edit_for` turns into edits against the content it is shown, and
reads a flattened `Video` for its timeline. The logic game, the logic grid, the
canvas and pixel art work the same way: the editor speaks in an operation enum
as a command, and `edit_for` on the content turns a command into an edit
against the content as it is now. The logic grid computes that by applying the
command to the grid it reads back and writing the fields that differ, so its
components merge field by field and its wires, stored as a set of unit-length
segments and a set of wire ends, merge segment by segment, so a cut, an
extension and a branch made at once all survive; they are joined back into
wires, split at the stored ends, when read. Two components given the same id at
once are both kept, the later one read with a fresh id. The canvas keeps its
layering as the order of its entity list, so bringing an entity to the front is
a move. Its entities keep their style field by field and their geometry
in general fields rather than a variant per tool, and an arrow end bound to a
shape is stored as the shape's id and an anchor and resolved when the canvas is
read (stopping a gap short of its outline), so it follows a shape a peer moved
without anyone rewriting the arrow
(`plans/canvas.md`, Where it stands). Pixel art keeps its pixels in a `Grid`. The database schema shows the other direction: its fields and enum
options are objects, and their ids are the ids a database's cells and enum values
store, so renaming a field or an option changes nothing that points at it.

A file is the other shape that does not fit a document: a small header and a
payload that can be megabytes. `Blob<K>` is that shape once, for any `BlobKind`
that names a content type and a header: `ImageContent`, `AudioContent`,
`PdfContent` and `GameModuleContent` are each a `Blob`
of their own kind. It encodes as a `Streamed`
type, its only live edit is `BlobOp::SetHeader` (the image editor records what
it decoded that way), and an offline merge takes whichever side changed it.
The payload never travels as an operation, because a session relays operations
in frames of at most 8 MB. It arrives as a commit instead, which be-store has
already cut into chunks: a new block's first content comes from `SeedContent`,
and a new file for an existing block from `ReplaceContent` (below).

Test a type's helpers in `crates/be-block/src/tests/`; the model itself is
tested in `crates/be-model/src/tests/`, and the round trip through a real server
in `crates/be-client/src/tests/`. An editor's tests stand in for the host with
`block_ui_test::BeuiTest`, whose `ContentStore` hosts every block a test holds
through one in-process `block_type_guest::Module` (every be-block type is
registered, and `hold::<C>` registers a type it does not know): one replica per
block, owned by this device, a store of commits that takes every publish, the test's
frame clock as the replica's time, and editors attached as endpoints on their first
`Replica` frame, the way the app's host does. `edit` and `apply_elsewhere` are edits
submitted by another participant, `set_peers` and `receive_from_peer` are presence and
channel messages from one; `store()` hands a test fixture the same `ContentStore` to
read or write between runs.

## Version control

Version control is two content types and a peer-side operation set, not a
store of its own: history is be-store objects and be-commit commits.

The model is jj's (Jujutsu's): the working copy is a commit, a change keeps its
identity when it is rewritten, conflicts are data, and every operation can be
undone.

- A **repository** (`be_block::Repository`) holds branch heads, an optional
  upstream repository in the same organization, and the operation log. It
  holds its history with `Peer::hold` (`HoldObjects`): a block that holds
  objects keeps them alive whatever its own commits' retention does, until it
  is collected.
- A **snapshot** is a `be_commit::Commit` of kind `Snapshot(description)` whose
  manifest is a `be_vcs::Tree`: one entry per block of the versioned subtree,
  keyed by local id, with its content type, parent, metadata, references and
  content manifest, and the tree's unresolved conflicts. The manifest is the
  block's own, so a snapshot of unchanged content stores nothing new, and a
  checkout publishes a block's first commit with that manifest without
  downloading it.
- A **change** is the line of snapshots that share a `ChangeId`
  (`Commit::change`). Rewriting a change (new edits, bringing the branch in,
  resolving) writes a new commit with the same change id; the commit id is the
  hash of the stored bytes and differs. A commit saved before change ids has
  none and is its own change (`Commit::change_id`).
- A **checkout** (`be_block::Checkout`) is the parent of a working tree of
  ordinary blocks, which every editor opens and every session edits as usual.
  Sharing the checkout block is how two people work on one tree; two checkouts
  are two people working apart. It records its repository, the branch it
  follows, `working` (the latest snapshot of the working change), root, the
  head each block had at that snapshot (so status is a comparison against the
  mirror rather than a download), and a view of each conflict (copies of its
  sides to look at). There is no staging area and no uncommitted state: edits
  are the working change, and every operation first snapshots it (amends it).
  Saving a change (`Finish`) describes the working change, moves the branch to
  it with a compare-and-swap, and starts a new empty working change on top.
- **Conflicts are data.** `be_vcs::merge` records every conflict in the merged
  tree's `conflicts` with the entry each side had, and carries conflicts the
  sides already recorded until one side resolves them. Bringing changes in,
  switching branch (the working change is rebased onto the other branch) and
  combining a branch (the working change gains a second parent) never refuse
  because of conflicts and never leave anything in progress: the result is
  the new working change, conflicts and all, which can be saved as it is.
  Resolving (`Tree::resolve`) is an ordinary rewrite of the working change.
- **The operation log** (`Repository::operations`, the latest
  `OPERATION_LIMIT`) records every operation: its action, the checkout it ran
  in, and the view before and after (the branches, and that checkout's working
  change and branch). Undoing one is itself an operation: it puts back the
  branches it moved, where they have not moved since, and reverts its effect
  on the checkout's working change by a three-way merge (the after view as
  base), so edits made since stay. Undoing an undo redoes.
- **Scopes.** A block's content stores the ids of the blocks it names, and
  those have to mean the same thing in every checkout, so they are *local* ids.
  A block copied into a checkout is created as `masked(local, scope_mask(checkout))`
  and carries the `LOCAL_ID` metadata key; a block created inside a checkout, or
  one that was adopted into it, keeps its own id as its local id. The host
  translates every block id that crosses the plugin protocol for an instance
  whose block is inside a checkout (`Instances::translate`, over
  `Message::visit_block_ids`), and the peer resolves the references it
  publishes the same way (`Peer::set_resolver`), so neither content nor editors
  know about checkouts. Resolution looks in the instance's own checkout first
  and falls back to the block with that id, which is how a reference out of the
  tree keeps working.
- `crates/block-app/src/be/version.rs` runs the operations in the worker:
  adopting a block into a new checkout, saving a change, making a checkout,
  switching, branching, combining a branch, bringing the branch's changes in
  (`be_vcs::merge` over the trees, then each content type's `Merge::merge3`
  for a block both sides changed, with `before`, `yours` and `incoming` copies
  of a conflicted block left under the checkout), resolving, undoing,
  forking, pulling and pushing. It pushes a `VersionStatus` for every
  repository and checkout an editor is showing, recomputed when the mirror
  changes; it watches the checkout's blocks and holds each followed checkout's
  tree in the mirror (`Query::Subtree` under `Owner::Version`), re-listing it
  before each operation so every head is current.
- A replica's owner reconciles when someone publishes past it (`HeadChanged`
  → `HeadEvent::Moved`), which is what lets a checkout replace the content of a
  block that someone has open; the worker sends `Moved` with `from_this_device`
  for what it publishes itself.

## Running it

```
./scripts/buck run //crates/be-server:bin -- --addr 127.0.0.1:9090 --data-dir be-server-data --allow-login --allow-registration
```

Open `http://127.0.0.1:9090/auth` to register. Until email delivery is connected,
verification messages are printed in the server log. Deployments also configure
`--auth-origin` with the public HTTPS origin (guides/hosting.md).

`--addr` (or `--address`) defaults to `127.0.0.1:9090` and `--data-dir` to
`be-server-data`. The desktop app does not need a server of its own for a local
account: it embeds be-server (`crates/block-app/src/platform/native.rs`) on an
ephemeral port with a data directory under the app's, and signs in to it like
any other server. An account on another server connects to that server's URL
instead, and the web build always does (`./scripts/buck run
//crates/block-app:web` starts be-server beside it, closed to new
accounts).

Tests start their own server on an ephemeral port; see `Harness` in
`crates/be-client/src/tests.rs` and `crates/be-server/src/tests.rs`. The app's
own tests do the same through the embedded server, register an account and
create an organization, and then start the app's peer against it the way the app
does: `Harness` in `crates/block-app/src/be/tests.rs`.

## How the app hosts content

The app is the peer, not the plugin. A plugin never sees a key, the
connection or the server; it is handed content and graph answers by the host,
and hands back operations and graph commands.

### The peer and its registry

The app's peer names no content type. Every block type runs in a type module
(guides/type_modules.md): a plugin's own, which its manifest's `block_types` name
and the app runs one instance of per plugin, staged beside the plugin as
`NAME.wasm` with its `.cwasm` and its SHA-256 (`NAME.sha256`, the tag of its
sessions); and `host_types::registrations()`, the settings types, the
repository and the checkout, which the app reads itself and runs in process
(`be::types::HostTypes`). What a type is called, its icon, color, child edits and
press come from the manifest that declares it, or for the host's own types from
`be::host_block_types`: discovery (`editors/plugin/discovery.rs`, `block_type`)
holds that catalog, so a type is known before its module has said `Hello`, and
refuses a plugin that declares a type someone else already has or edits one
nobody declares. `be::provided` is whether some module will hold a content type,
which is what the plugin host checks before following a block's content.
The registry (`be_types_host::modules`, re-exported by `be/types.rs`, which
lists the app's module sources) holds each module's `Hello`, with every type's blank
content and its description asked for once at start, answers `be::kind_of` and
`be::is_known` (frozen, schema fingerprint, blank, which module). Everything else
about a type, merging, describing, validating bytes, is a request to its module,
which the worker awaits without blocking (`Replicas::merge`, `describe`). A block
whose content type no module has has no content in the app.

### Blocks saved by another build

Until a content type is frozen its format changes freely, so a block may hold
what an older build saved. Every save stamps the derived `SCHEMA` key with
`schema_fingerprint::<C>()`, a hash of `BlockContent::schema()`: be-model's
`Kind::describe` for a document, and `blob N` for a blob, whose kind bumps
`BlobKind::SCHEMA` when its header changes in a way old headers would be read
wrongly. `be::staleness` calls an unfrozen block stale when its replica could
not read it (`Status::unreadable`, kept in `Shared::unreadable`) or its stamp is another schema's; a block
with no stamp is not stale. A stale block's frame shows `StaleNotice`
(`surfaces::StaleBlock`) instead of its editor: Reset publishes the type's
default content (`be::reset`), Delete deletes it as the shell would, and Try
anyway shows the editor for the rest of the session (`be::try_anyway`). A block
the app holds itself (`be::hold`: settings, the shell's session) is reset by
the worker without asking, with a notice, since the app needs it to work: it
publishes the type's blank content and opens the replica again. The
app deletes `plugin-cache/` whenever its build id changes; it keeps
`be-objects/` (objects are content-addressed and frozen) and the refs database.

`crates/block-app/src/be/worker.rs` is the peer's loop and
`be_types_host::Replicas` (shared with `be`, guides/type_modules.md "Hosts";
`be/replicas.rs` is the app's `Host`, where reports land in `Shared`) the host
half of every open block's replica: it opens a replica (joins the session,
watches the block, sends `Open` with the head and `Moved` for a later one), answers
its commands with the real primitives (`Read`, `Publish` through
`Peer::seal_encoded` then `publish_sealed`, `Stash` through `stash_encoded`, `Resume`, the lease, relaying
sealed with the block key, `WakeAt` from the clock), and keeps what it reports
(`Status`, `Described`, `History`, `Conflict`) in `Shared`. Natively the module
runs on a thread of its own (`PureHost::spawn`), on the web in a web worker
(`WebGuest`), and in the unit tests in process (`InProcess`); the worker never
waits on one, since every module command and IO answer is an event in its loop.
A module that stops (a panic, or a step past `STEP_TIMEOUT`) is started again,
and every replica it held is opened again from its last head, with the editors
attached and the messages relayed since its last seal replayed. `be/native.rs` runs the loop on a thread of its own with a `FileStore` under it,
and `be/web.rs` runs it in a web worker of its own with the same `FileStore`,
whose directory is in the origin private file system (guides/running/web.md).

The app holds one connection per signed-in account and server, in
`crates/block-app/src/link.rs`, and keeps it until it closes. Sign-in hands its
connection over (`link::adopt`); the account requests in `accounts.rs` and the
worker's `Peer` borrow it (`link::signed_in`), which reconnects and
authenticates again only once it has closed. Natively it runs on a runtime of
its own, so it outlives the request that opened it. The server keeps one open
organization per connection, so only the worker opens one, and a worker waits
for the last one to stop before it does (`Link::open_organization`). Opening it
again, even the same one, drops the connection's watches and listings, so what
needs the open organization's verified escrow key (inviting, creating an agent)
goes through the worker's `Peer` rather than opening it.

A `Peer` keeps what it learns in a local refs database (`be_client::refs`,
`RefsDb`, passed in `PeerConfig::refs`; an in-memory one by default), keyed by
server, account and organization: each block's local head and last-known remote
head, the raw `BlockSummary` of every listing (metadata stays sealed), the key
rows and wraps it was served, its verified contacts and escrow chain, the
workspace list (`RefsDb::workspaces`), an outbox and an unsent area. It is
SQLite on every platform (`RefsDb::open`), in the origin private file system on
the web. `Peer::cached` builds a peer from it with no
network, and `Peer::attach` hands it a signed-in connection later (again after
every reconnect), which opens the organization and refreshes keys and contacts.
While the connection is down, reads fall back to the cache: `summary`, the
listings and `read_contents` answer from it on `ClientError::Disconnected`, and
`open` reads the local head first while it has unpublished commits.

The app keeps one refs database per device, `be-refs.sqlite3` beside
`be-objects/` in the data directory (`be::refs_in`, `/block` on the web). The worker builds its `Peer` once per workspace: with
`Peer::cached` when the database already holds a session for that server,
account and workspace, so the graph, listings and open blocks come up with no
network, and otherwise with `Peer::signed_in` on the first connection. Every
later connection is handed to the same peer with `attach`. The workspace list is
cached there too (`RefsDb::set_workspaces` after each load); when the list
request fails the app shows the cached one and loads it again once the worker
connects or the window gains focus.

Saving is two steps: `Peer::seal_manifest` writes the commit, records it in the
outbox and moves the local head, and `publish_sealed` sends it; `publish_manifest`
does both, and on `Rejected` drops the commit from the outbox since its caller
retries or merges. A commit whose publish fails offline stays in the outbox, as
does a `create_block` made offline (`refs::Op::Create`). `Peer::drain` publishes
the outbox in order once attached and reports, per block, `Drained::Published`,
`Created`, `Moved`, `Defined`, `Diverged { local, remote }` (left in the outbox for the replica host to
merge: a publish whose parents include the local commit settles it), `Refused`
(an operation the server turned down for another reason, dropped) or `Unsent`.
Graph operations queue the same way while detached: `set_parent` (moving and
deleting) queues `refs::Op::Move` and `define` on a block with no content
`Op::Define`, and both rewrite the cached summary so listings show the change
offline; while any of these wait for a block, later ones queue behind them.
`define` on a block with content seals a local commit. `unlist` is a no-op
offline. Sharing, access lists, members, invites, agents, contacts and pairing
need the server, and the worker answers them offline with "This needs a
connection to the server".
An outbox entry is settled once a published head descends from it. A block
whose access is refused (on a publish, a create or `summary`), or whose summary
arrives without edit access, moves its entries to the unsent area
(`Peer::unsent`, `restore_unsent`, `discard_unsent`); nothing there is ever
deleted except by `discard_unsent`, and `UnsentBlock::offerable` says the cached
summary allows editing again. `Peer::pending` counts outbox operations, local
commits, their blocks and unsent blocks.

### The graph mirror

The app keeps a mirror of the part of the server's graph something in the app
is looking at, in `be/graph.rs`: a `Node` per block with its content type,
author, parent, access, references, unsealed `BlockMetadata` and whether no key
it holds opens it (`locked`). A block the account may only know exists has no
author or parent, so `Roots`, `Detached`, `Shared` and `Children` never list
it; it is reached only through what references it (`ReadBlock` answers it with
that much), and is never `locked`. The app never lists the whole organization.
`Query` (`Roots`, `Detached`, `Shared`, `Children`, `References`, `Backrefs`,
`Parents`, `Block`, and the host's own `Subtree`) is how everything in the app
asks the mirror a question, and `be::graph_revision` says when to ask again.
Whoever asks first says what it wants with `be::want(owner, queries)`: each
plugin instance its `WatchBlocks` queries, its own block's `Parents` and the
blocks whose content it watches (`Owner::Editor`); the worker the parents of
every open block and of every revision preview's source (`Owner::Open`) and the
roots (`Owner::Host`, for Settings and My files); version control its
checkouts' subtrees (`Owner::Version`). After every change the graph turns
what is wanted into `Listing`s - a parent's children (`ListChildren`), the
blocks shared into view (`ListShared`), a block's backrefs (`ListBackrefs`), the
blocks a block references (`ListReferences`, one request however many there
are; the server stores references already resolved to real ids, checkouts'
included), one block (`ReadBlock`) - expanding `Parents` and `Subtree` as the
blocks they name arrive, and listing a block's references again when they
change, and the worker fetches the new ones together
(`fetch_listings`; its own loop spawns them as a task, `spawn_listings`, and is
woken when they land, so a listing a silent server holds up never holds up an
open or an edit). A listing nobody wants any more is dropped, and with it
every block no other listing holds, so closing a folder in the files pane
empties it from the mirror; the worker sends `Unlist` for it, ahead of that
round's new listings, so the server stops announcing it. A block already held
is asked for on its own all the same, so the server keeps announcing it once
the listing that brought it is gone, but its query is ready at once. A query is answered only once its listings have
loaded (`be::ready`), so an editor never sees a folder as empty while it
loads. The server's `BlockChanged` and `BlockRemoved` events keep what is
listed current: a block is kept only while a listing holds it (a new child of a
listed folder joins that listing, one that moves out leaves it), and news of a
block shared into view re-lists `Shared`. Reconnecting, or falling behind the
event broadcast, re-lists everything wanted. The mirror remembers the newest
revision it has heard of each block (and, for the rest, that of the newest
listing) and drops anything older, so an announcement the worker reads late
never undoes a newer reply. `Shared` is the account's entry points: blocks it
can view whose parent it cannot.

A change made here - `be::create`, `be::set_parent`, `be::define` - is
applied to the mirror straight away, so the file tree and every editor see it in
the next frame, and counts as pending for that block. While a block has a
pending change the mirror ignores the server's echoes for it, which would
otherwise show a state older than the one on screen; the worker settles the
change when the server has answered, and refreshes the block from the server
once nothing is pending. An open block's references come from its session's
content rather than the server, so a child added to a folder is listed before
the edit is sealed.

### Metadata

A block's metadata (`be_block::BlockMetadata`) is two maps from a key's UUID to a
CBOR value: `defined`, what someone chose (a name typed by hand, the
`Artifact` naming the block a dynamic artifact was made from, a checkout copy's `LOCAL_ID`),
and `derived`, what the content says about itself (`INTRINSIC_NAME`,
`THUMBHASH`, `SCHEMA`). A key is declared once as a typed constant,
`MetaKey::defined(id, name)` or `MetaKey::derived(id, name)`, and read and
written through it (`get`, `insert`, `remove`, or `definition` for a change to
send), so a new key, including one a third-party block type declares, needs no
change to the format. A value that holds a `be_model::BlockRef` points to that
block: `BlockMetadata::references` finds every one, and the peer adds them to
the references it publishes with the block (`Publish`, and `CreateBlock` and
`SetMetadata` for a block with no content yet), so the graph's backrefs and
`KnowExists` see them as they see content's, and a parent holds a child its
metadata points to (a settings block, an edit kind). They are not the block's
children, though: the host hands plugins only what the content refers to, as
`BlockInfo::references` and `BlockQuery::References` (block-app `Node::contents`),
so a canvas or a folder never lists the tags or settings its metadata names; read
those from the metadata, and `BlockQuery::Backrefs` finds the blocks pointing to
one either way. `BlockMetadata::name` is `NAME` when someone named the
block and `INTRINSIC_NAME` otherwise. Plugins get both maps in `BlockInfo`, read
them through its `name`, `thumbhash` and `artifact`, and set a defined key with
`Blocks::define`; the host never lets a plugin set `LOCAL_ID`. A beui
`unstyled::Picture` given an image's thumbhash as a `beui::Thumbhash` lays out
at the image's size and paints the blurred placeholder until the image arrives.

Metadata travels with content. Every commit carries the encoded metadata of the
content it saves, and `Publish` carries it sealed with the block's data key
(see Keys), so the server stores it in the same step that moves the
head and cannot keep metadata that belongs to a head it refused. A save takes
`defined` from the head it builds on and recomputes `derived` from the content
(`be_block::derived_metadata`: `BlockContent::name` and `BlockContent::describe`;
a blob type describes itself from its header alone, so an image's thumbhash is
copied out of `ImageHeader`, never computed again). A merge after a refused save
merges `defined` key by key against the base commit
(`BlockMetadata::merge_defined`, the head already published winning a clash) and
derives the rest from the merged content; version control does the same, and a
clash on `NAME` there is `ConflictKind::Name`. Defined keys a build does not
know ride along unchanged; derived keys it does not know are dropped when it
saves new content, since they describe the old content, and filled in again by
the next build that knows them.

`Peer::define` changes defined metadata: on a block with content it publishes a
commit of the current content with the new metadata, which a live session then
catches up with like any other publish from outside it; a block with no content
yet takes it through `SetMetadata`, which the server refuses once the block has
a head. In the app, `be::define` (and `be::set_name`, the `NAME` key) applies
the change to the mirror at once and sends it. `be::describe_implicitly` keeps
the derived metadata of an open block current in the mirror as its content
changes (`Graph::derive`), until the save that publishes it.

### Content on the plugin protocol

An editor is a follower of the block's replica (guides/type_modules.md, "Editors
are endpoints"), and everything about a block's content crosses the plugin protocol
in one message, `EditorMessage::Replica { block_id, frame }`, in both directions.
`frame` is one `block_type_api::encode_frame` of a `block_type_guest::editor::ToEditor`
(host to plugin) or `FromEditor` (plugin to host) (`ToEditor::frame`, `from_frame`), and
the host relays it unread between the instance and the block's replica.

- Plugin to host: `Operate`, `Gesture` (`Begin`, `Commit`, `Cancel`), `ShowPresence`,
  `SendToPeers` and `Resend`. An instance asks for a block's content by sending
  `Resend` when it first follows the block, and the host attaches the instance to the
  replica on the first frame it sends for a block (its own or one in `WatchContent`),
  with `editable` from the account's access, which it sends again when that changes.
- Host to plugin: `Snapshot { content_type, bytes, session, unsettled, applied }`, the
  settled content and the operations still unsettled on top of it; `applied` counts the
  operations *this instance* sent that the snapshot already holds, which is what lets a
  plugin tell its own unacknowledged edits from everyone else's. `Operations`, every
  operation since the last frame, in order, each marked `mine` when this instance sent
  it, with how many operations are still unsettled after it (`unsettled`) and, for a
  peer's operation that goes beneath those, the unsettled operations as rebased
  (`beneath`). `Presence` and `PeerMessages` from the other participants.
- `EditorMessage::WatchContent { blocks }` - plugin to host, the other blocks (and their
  content types) the editor wants to follow. The host detaches the instance from a
  block it drops, and from every block when it closes.
- `SeedContent` and `ReplaceContent` write a block's content outright as host requests,
  since they may name a block nothing follows; `ReplaceContent` becomes the replica's
  `Change::Replace`.

`editor.content_of::<C>(block)` is how an editor reads and edits another block:
it registers the block with `WatchContent` and hands back the same
`ContentProjection` an editor gets for its own block, keyed by that block. The
host keeps a content link per block per instance beside the editor's own, opens
a block's replica only when an instance first sends it a frame (a frame that
arrives before its `WatchContent` waits for it), attaches it read-only unless
the account may edit the block, and closes the replica when no instance and
nothing in the app holds it any more. A revision preview and a block still
uploading from an import are answered with a `Snapshot` the host makes from the
bytes it has, with no session state, since no replica holds them. An editor
that follows a block chosen by its content rather than a fixed one uses
`editor.related_content::<C>(block)`: given a `Memo<Option<Uuid>>` it projects
whichever block that currently names. A database view follows its database and
the database's schema this way.

Operations, not snapshots, are the normal case, and how they settle (a
follower's own edits stay unsettled until the owner accepts them, a peer's
operation the owner sequenced first goes beneath them) is the replica's:
guides/type_modules.md.

On the plugin side that is `editor.block_content::<C>()`, one `ContentProjection`
per editor, built on `block_type_guest::editor::EditorMirror<C>` (which keeps the
settled content, the unsettled and pending operations, what an update touched,
`revision()` and the `Projected` log `LiveText` reads), plus the reactive watchers. `operate` applies an operation to what the view sees and queues it
for the host. An operation that comes back `mine` only confirms what is already
shown; one from anyone else is applied on top of it, unless an edit of this
instance's is still in flight, in which case it goes beneath that edit
(`LiveEdit::apply_beneath`), as a `Beneath` operation goes beneath the host's
unsettled operations and this instance's pending ones together. Only an
operation from another instance that the host itself has not settled, arriving
while this instance has an edit in flight, rebuilds what the view sees. Applying an operation reports what it
touched (`LiveEdit::apply_touching`), and a projection only runs when something
it watches was touched; `guides/reactive.md` says how `project`, `field`, `ids`
and `object` narrow that. `ContentProjection::read` answers `Err(Pending::Loading)`
until the host has sent the content once, `operate` drops an edit made before
then, `ready()` is that state on its own (`guides/reactive.md` says what an
editor shows until it is `Ok`), and `revision()` counts the changes an editor has seen, for code like the
PDF pane that re-renders on a change rather than projecting.

`block_editor_beui` re-exports `be_block`, so an editor names its content type
without depending on the crate itself.

### The graph on the plugin protocol

A plugin asks about the graph with `EditorMessage::WatchBlocks { queries }`, a
list of `BlockQuery`s the host answers from its mirror with `Blocks { query,
blocks }` whenever the answer changes (`plugin_host/graph.rs`). A `BlockInfo`
carries the block's type, author, parent (`BlockLocation`: `Root`, `Detached` or
a block), name and whether it was named by hand, references, the account's
access, the artifact source and whether it is `locked`: the account may view
it but holds no key that opens it yet, so its metadata is empty. A block the
account may only know exists (`AccessLevel::KnowExists`) comes with no author
or parent and nothing else but its type. `BlockInfo::reach` tells the two
apart (`block_ui::Reach`: `Open`, `Locked`, `NoAccess`), `BlockInfo::label`
names such a block "Locked" or "No access" in place of its name, and
`block_editor_beui::label_glyph` gives either a lock icon, so every place that
draws a referenced block from its label (text links and embeds, the files
pane, database cells, canvas cards, pickers) shows it; a frame says so in
place of an editor. `BlockInfo::deleted` is set on a block whose parent chain
the host's mirror follows up to a link that does not hold (`Graph::deleted_root`):
it is in Recently Deleted, itself or inside a deleted folder, though links to it
still resolve. Where the mirror cannot read a parent's references, it takes the
server's word (`BlockSummary::held`); a chain that runs into a block the account
cannot see counts as live. Recently Deleted is `Listing::Deleted`, the
blocks whose own link does not hold, and a publish that changes a block's
references re-announces the blocks it dropped or added so the listing follows. `BlockInfo::label` carries the flag, `label_glyph` gives such a block a
trash icon, and the places that list references treat it as a link that will
break: the files pane mutes it (but not inside Recently Deleted, where every
row is deleted), the shell's reference menus and phone details sheet list it
last as "In Recently Deleted" and leave it out of the counts and the linked
bar, pickers never offer it, and a deleted block opened in a tab has a bar
above it saying so. Anything may ask the host to bring one back with
`EditorHost::restore_block` (`BlockCommand::Restore`): the host finds the
block at the top of its chain and lists it in its parent again when that parent
is live, editable and can add a child; a block with no parent goes back where
the `DELETED_FROM` metadata key says it was deleted from (which `be::set_parent`
records when it detaches a block, as deleting a top-level block does, and clears
when it gives it a parent again), and otherwise into My files. It changes the graph with
`CreateBlock { block_id, content_type, parent, name, artifact, content }`,
`SetParent` and `SetName`, which the host takes only when the account may edit
the block and the parent it names, and hands to `be::create`, `be::set_parent`
and `be::set_name`. On the plugin side these are `editor.watch_blocks(query)` and
the `Blocks` handle from `editor.blocks()`.

### Creating, seeding and replacing

A new block's first content travels with it: `CreateBlock` carries the encoded
content, and the worker creates the block, then writes the content as its first
commit, before any later command for that block runs. With no content the
worker writes the type's blank content, which its type module made at `Hello`
(`Kind::blank`), so a host or plugin that makes a block of a type whose format
it does not own names only the type's id: `Blocks::create_with(type, None, ..)`
for the file tree's New workspace, `be::create(.., None)` for My files and a new
session's view. `Creation::create` and
`Blocks::create` make a block this way, Detached unless a parent is named;
the host sets the parent of a block made in a creation dialog once it has the
id. Creating a database makes two blocks this way:
`block_editor_beui::database::create_database` creates a schema with a Name
field and a database pointing at it, and makes the schema a child of the
database, so the graph is right before any editor opens it.

`editor.seed_content(block, &content)` sends `EditorMessage::SeedContent` for a
block that already exists; the peer writes it as the block's first commit, and
ignores it for a block that already has content or is open, so it cannot
overwrite anything.

Replacing a block's whole content is `editor.replace_content(block, &content)`,
which sends `EditorMessage::ReplaceContent`; the host takes it only for a block
the account may edit. The worker hands it to the open replica as
`Change::Replace`, or publishes it straight to the server when nothing has the
block open. The replica publishes the new content as a commit based on the head it
knows, retrying on the newer head a rejection names, so the bytes go through the
chunked upload rather than the session. An owner that replaces settles on that
commit and tells its followers to reload; a follower sends the owner
`SessionMessage::Replaced { head }`, and the owner takes that content. The
replacement wins outright: whatever a replica had not saved when the replacement
reached it is written as a stash commit that becomes a parent of a later head, so
it stays in the block's history, and nothing is replayed on top
(guides/type_modules.md, "Peers"). The image, audio and PDF
editors' "replace" buttons and the pixel art export's regeneration all go this
way. `content_file_creation::<C>` is the creation view that makes a block of a
file type from the file the user picks.

A block file (`be_client::transfer`, the file `be download` writes) carries a
block and everything under it. A plugin asks the host for one with
`HostRequest::ExportBlock`, answered like `SaveFile`: the worker lists the tree
a level at a time (starting from the view when asked for a database inside
one), all of a level's `ListChildren` together, and fills in each block's
bytes, from its open session or else its head, so it works for content types
the app does not know.
`Peer::read_contents` reads the heads, which `be download` uses too: one
`ReadContents` request per 256 blocks answers each block's summary with its head
commit and chunks (those that fit in half a frame, and none for a head the
client says it already holds), and anything left out is fetched afterwards, so
a tree costs one round trip, not three per block.
`HostRequest::ImportBlocks(parent)` has the host pick a file and `be::import`
it: every block gets a new id, old ids in the content bytes and references are
swapped for the new ones, and each block is created and given its bytes as its first commit. The
reply, `BlocksImported`, names the outermost block. The tree is in the mirror
at once and uploads in the background, as a task beside the worker's loop
(`worker/uploads.rs`), so commands for other blocks are not held up.
`Peer::import_blocks` is `queue_import` then `upload_import`. `queue_import`
mints every block's keys and writes its commit locally, needing the server only
for the scope of the folder it goes into, and keeps each block in the outbox
(`Op::Create` and `Op::Publish`, with the commit as its local head), so an
import made offline or cut short by a quit is sent by the next drain;
`upload_import` then removes each block's entries once the server answers, and
sends
the objects as `PutObjects` and every create and publish without waiting for
the ones before: a tree costs a few round trips, not a few per block. `be
upload` and the pages of `be import` go through it too. A
command that needs a block still uploading waits for it, and one that rekeys
everything below a block (moving, sharing or leaving it) waits until all of
that has uploaded. An editor of an imported block
is sent the bytes from the file as a snapshot until its replica opens, so it
needs no round trip to show. The picker imports straight into the folder it
places in, since moving a tree into another workspace rekeys every block in it.

### Children, duplication and undo

Moving a block into a container, deleting a child or replacing it with a copy
asks the container's content to change. `PluginEditor` sends that to
`be::change_child`, which opens the container's replica if nothing has and sends
it `Change::Child`; the content type answers with `Root::child_edit`, which
turns a `be_block::ChildChange` into an ordinary edit: a presentation adds or
drops a slide, a database clears or rewrites the cells that link the block, a
hotbar unpins or repoints a component. `change_child` answers `None` only for a
content type no module has.

Duplicating a block is `be::duplicate`: it adds the copy to the mirror with the
source's type, references and metadata (without its artifact source), and the
worker copies what the source's replica shows (`Request::Content`), or its head
when it is not open, into the copy's id before anything opens the copy.

The app can read a block itself, not only through an editor:
`sync_local_settings` hands the input settings (keymap, key repeat, and
pointer settings for every pointer with per-device overrides) to the `--session`
seat through `beui_adapter_drm::InputControl` and to the nested Wayland clients'
keyboard, and the display settings to the app and the seat. `DisplaySettings` is
one block for everything about what the device shows: the theme, motion and scale,
which `sync_ui_settings` takes from the display settings last applied (the theme,
motion and canvas touch are the app's layer of beui's preferences, so an unset one follows
the system's, guides/beui.md; canvas touch, which a canvas's dock turns off, is kept on
this device only; the scale is beui's zoom factor, multiplied into the screen's own pixels
per point); a
`ScreenSettings` per screen (a mode and a rotation, keyed by the monitor's EDID
make, model and serial, or by its connector when it has no EDID), whose modes and
rotations go to the seat's outputs through `beui_adapter_drm::DisplayControl`,
which switches both live (guides/linux_desktop.md says how a screen is turned),
and which reports each monitor's applied rotation with its modes; whether the
bar shows a rotate button; and when the screens turn off and lock. Every setting is an `Option`, and an unset one means the default:
libinput's own for a pointer setting, which the seat reports per device and an
editor reads as the `InputDevices` host value, and for a monitor its preferred
resolution at the fastest refresh rate it offers there, which the seat reports
with the monitor's modes and an editor reads as the `Displays` host value
(guides/adding_a_plugin_editor.md says how host values reach a plugin). The seat, the stored settings and those host values share one set of types from `ui-types` (`DisplayMode`, `Rotation`, `InputDevice`, `PointerSettings`), so nothing converts between them. Because the seat runs before anyone signs in, the app
keeps a copy of the last settings of each kind it applied in `app_state` and
applies that at startup, the theme and scale with it, even before anyone signs
in; the block stays the source of truth. A kind of setting
the app applies itself implements `local_settings::LocalSettings` and is kept by
a `SettingsSync`; its `apply` answers whether the settings may be saved now.
Display settings may not until someone keeps them: a change of mode that moves what a
connected monitor shows (a rotation applies and saves at once), or a monitor showing a mode
other than its preferred one that this device never kept it at (an untried fastest default, or
a mode another device chose for that monitor), opens the host's "Keep these display
settings?" prompt (`styled::KeepChanges`, a card on every screen) in
`display::guard`. It is the host's because it must work before any shell plugin
runs. Each change is a round (`Guard::prompt`): another change while asking
starts a new round and a full countdown, and an answer counts only for the
round it was given in. Keep saves, storing a kept default as the monitor's mode;
Revert, Escape or 15 seconds of the frame clock, which the guard counts itself,
put back the earlier modes, or the preferred mode for an untried one. The modes each
monitor showed once answered are this device's own memory (`app_state`'s
`tried_display_modes`), not a setting. The guard answers with modes per screen, which
`display::keep` and `display::revert` write by the layer rule below (a fallback from a mode
another device chose goes to this device's profile, so it leaves the others alone), each as
an edit on that layer's own content (`SettingsSync::layers`), so a screen's register in
a block never picks up a rotation it only inherits, and `SettingsSync::commit` applies
them to their layers and layers the result again. It writes
those edits into the block too, holding them until its content is loaded and reading
the block with them on top until a revision holds them.
`be::hold` opens a
block for the app and keeps it open when the last editor showing it closes,
because `be::close` leaves a held block alone. Nothing releases a held block
before the stack stops, which is when the organization changes.

Undo lives in the block's replica, not in an editor, because it is the one place that
sees every edit to a block from this device, whichever editor made it. A content
type registered with `undo` in `be_block_types!` gets a replica that keeps an undo and a
redo stack (`be_block::history::History`): every operation an editor sends records a step, a step sent
within 750 ms of the last one, by the time the host ticks the module with (the
frame clock's instant each relayed frame is stamped with), may be absorbed into
it, and `be::undo` and `be::redo` send the replica `Change::Undo` and `Redo`.
An editor groups the edits of one gesture into one step, however long it takes, with
`FromEditor::Gesture` (`Gesture::Begin`, `Commit`, `Cancel`; on the plugin side
`ContentProjection::begin_gesture`, `commit_gesture`, `cancel_gesture`): every edit from
that instance until the commit folds into one step (`Undo::then`, which keeps one value
per register however many frames wrote it), and a cancel reverts the step and leaves
nothing in the history. The replica publishes only once its edits have rested
for the seal interval, so a gesture written every frame costs one publish, not
one per frame.
Plugins reach it with `BlockCommand::Undo` and `BlockCommand::Redo`, and watch
whether either is possible with `EditorMessage::WatchHistory`, which the host
answers with `HistoryStates` whenever they change. block-ui-test's replica keeps the same
history (with the 750 ms bursts on the test's frame clock) for a block a test calls
`ContentStore::keep_history::<C>` on, and none otherwise.

### Organizations and workspaces

The account switcher picks an account, the Organizations screen one of its
organizations (it opens the only one by itself), and everything below happens
inside that one organization. A workspace is any root block
(`BlockParent::Root`) that is not a system block
(`block_editor_beui::sections::is_workspace`); its name is its `NAME` and its
members are the grants on it. New ones are of the workspace type
(`be_block::WORKSPACE_TYPE`, whose format, shaped like a folder's, an ordered
list of children, is `workspace_ui_types::Workspace`), made blank by its type
module. Only workspaces and an account's own
`Settings` belong at the root: nothing places another block there, and a block
deleted from the root that cannot hold children is restored into My files. The
server still accepts any block at the root, so an older one left there shows
as a workspace. Opening an organization makes sure the account has a "My files"
workspace it authored, marked with the `MY_FILES` metadata key
(`RootSettings::ensure_my_files`, beside the settings it ensures).

The files pane (crates/editors/file-tree) shows a section per workspace the
account can see (My files first, then by name; another account's My files is
never listed), then Shared with me (`BlockQuery::Shared`), then Recently Deleted, then
System: the account's own internal root blocks, the content types in
`SYSTEM_TYPES` (today `Settings` and settings profiles), which open in their
editors and expand to what they hold; another account's are never listed. It is
there for debugging: settings are reached from the workspace's Settings tab,
beside Files, which shows the account's `Settings` block in the settings editor,
a list whose rows open each kind (or, behind Devices and profiles, each profile) in a
tab of the editors pane. A
section's header has its own menu (add, rename, share, delete, and leave on a
workspace someone else made) and, where the account may edit the workspace, a
"+" that adds a block into it, as a row's "+" adds into that row; the toolbar
has no add of its own, only New workspace, which makes one at the root and
opens the rename dialog. Every picker places a new block in a workspace, My
files when nothing else is named.
A block moves between workspaces with the host's `move_block`, by dragging it
or from its Move to menu. A phone shows the same tree with taller rows, under a
title with New workspace and the account button in place of the toolbar.
Pressing a folder, a workspace or one of the other sections zooms into it rather
than opening it: the tree then holds only what it holds, under a bar with the
path, a back button (or, on a phone, a swipe back) and a New that adds into it
where the account may. Every row's menu has Zoom in too, so a block that holds
others without being a folder, a text block with a subtree, can be zoomed
into as well. The folder editor (crates/editors/workspace-index) still edits
folders and workspaces wherever they are embedded, and opens from Open with.

### Settings, settings profiles and sessions

Each account keeps what is its own in an organization under one top-level
`Settings` block it authored. Every reader finds it by its author: an organization
administrator can see every block, including other accounts' `Settings`, and
nothing may pick those up. `root_settings::own_settings_blocks` (plugins) and
`RootSettings::owned` (block-app) list the account's own, and when two devices
each made one before seeing the other's, both are read as one
(`Settings::merged`, the lowest id winning a clash) and written through the
lowest. The settings editor shows someone else's `Settings` or settings profile
with a notice, and makes and changes nothing in it. The editors of the kinds lay out
their pages with beui's `SettingsPage`, `SettingsSection` and `SettingsRow`: a
card of rows per section, each a label and description beside its control,
stacked once the row is narrow, and a section runs edge to edge once it is. A
`modified` row has an accent bar at its edge; given `menu` items, a "More" menu button
at its end holds what can be done to the setting, resetting it included (rows have no
other reset), and a `note` is a small accent caption under its description. A `SettingsLink` is a row that opens a page of its own (a block,
in a tab): its label and description, a chevron where a control would be, and the
hover a button has, inset and rounded so it stays inside the card, marked when
`modified`. A reset that deletes the block a link opens belongs to the setting that
chose the block, whose menu asks first (the Remapper row asks before it deletes the remaps).
A page shows a section only where the host can back it: the settings plugin's
Display page (`editors/settings`, `display`) always has Appearance, but a
section per screen only once the host offers `Displays` (its mode and its
rotation: four tiles, each a little screen drawn turned by that rotation less the
one the host reports now, so the tile that looks upright to the person holding the
device is the one to pick, named Landscape, Portrait and their flipped turns from
whether the panel is wider than tall; `block_editor_beui::RotationPicker`, shared
with linux-desktop's bar), a Screens section with the switch for the bar's rotate
button (`display.rotate-in-bar`) once the host offers `Displays`, and Power and lock only
once it offers `Power`, which only a `--session` publishes. Its Input page
(`input`) shows its sections only once the host offers `InputDevices`, which
likewise only a `--session` publishes, and says so otherwise.

Each kind of setting (`InputSettings`, `DisplaySettings`,
`EditorSettings`, the logic hotbar) is a block of its own, and is read through layers:

- The `Settings` block's `kinds` map a kind's content type to the block that
  holds it for every device.
- A settings profile is a `SettingsProfile` block, a child of the `Settings`
  block listed in its `profiles`, with `kinds` of its own and a `class`, the kind
  of device (`DeviceClass`: Desktop, Phone, Tablet, E-paper tablet). Each device
  (`device_profiles`, keyed by the client id) uses one, and block-app makes one
  the first time a device opens the organization, named after its class ("Browser"
  on the web).

`Settings::layers(kind, profile)` names a kind's two blocks (`Layers`: every
device, this device), and `be_block::layered` folds their contents with the kind's
`Layered::over`: what the profile sets wins field by field, and what it leaves unset
falls through to the settings for every device, then to the built-in default. Every
field of a layered kind is an `Option` for this. `Editor::effective_settings` gives what
every layer resolves to, which is how workspace-ui reads `EditorSettings`: the default
editor of each block type, keyed by the editor ids the host's catalog lists (see
guides/adding_a_plugin_editor.md), and `Editor::inherited_settings` what the layers
below an editor's block resolve to.

A block may have settings too, which apply to everything inside it: a workspace's,
a folder's, a game world's. They are a `Settings` block whose parent is that block,
pointed to by the block's `SETTINGS` metadata (a `BlockRef`, so the block holds it),
with one child block per kind in its `kinds` like the user's. Only a manager of the
block makes, changes or lets go of them (be-graph's `level_below`, be-server's
`require_parent_of` and `require_keeping_settings`). Each kind declares on `Layered`
its `NAME`, whether it follows the block or the viewer (`FOLLOWS`: theme and input
follow the viewer and never read a block's settings; the logic hotbar follows the
block), whether a block's value is a default the user's layers override or a policy
that fixes what it sets (`RULE`), and who may read it (`VISIBILITY`). Resolution
(`be_block::Resolution::resolve`) stacks, from the bottom: the organization's defaults,
the nearest block with settings for that kind, the every-device block, this device's,
then whatever is a policy (a block's, for a policy kind, and the organization's), and
says which layer set a field (`source_of`, `locked_by`). `Editor::kinds_above(block)`
walks a block and its ancestors, stopping at the first the viewer can't view, and keeps
the nearest kind blocks per block for the editor's life; `Editor::resolved_settings`
keeps the resolution per block and kind; `Editor::effective_settings` is the resolution
for the editor's own block; `Editor::organization_layers` is where the organization's
two layers will come from, and gives none yet. `Editor::ensure_block_kind` makes the
settings block and the kind, and `LayeredSettingsRow` says "Set by your organization"
or "Set in *Folder*" for a field another layer fixes. A kind block granted View for
the pseudo-account `be_graph::ANYONE_INSIDE` is readable by anyone who can view a block
inside the settings' owner, not only by its members (`BlockGraph::views_inside`); the
settings editor switches that grant per kind. Past the first ancestor the viewer can't
view, the walk takes those from `BlockQuery::OpenSettings(block)` (the server's
`Listing::OpenSettings`: the nearest open kind of each type above), and a `SetKind`
found there names its settings block as its owner, since the owner is hidden.

Two kinds change with `Edit` on their block rather than `Manage`, since anyone who
adds a database or tags a block in a workspace changes them: `PrefixRegistry` (which
databases use each row id prefix) and `Tags` (the tag definitions). A settings block's
`kinds` only a manager writes, so these are children of the block itself instead,
each pointed to by a metadata key of its own (`be_block::edit_kind_key(kind)`:
`PREFIXES`, `TAG_DEFINITIONS`), which holds it; `kinds_above` reads both places, the
file tree and the picker never list them, since metadata references aren't children,
and `Editor::ensure_block_kind` makes either. Both default to anyone inside. A tag is
a block of its own (`Tag`: a name and a `TagColor`) listed by `Tags`; a block carries
the tags put on it as `TAGGED` metadata (references, so "every block with this tag" is
`BlockQuery::Backrefs` on the tag), and the user's own tags are a `Tags` kind in their
settings, resolved together with the workspace's (`Editor::available_tags`). `Home`
(a manager's kind) holds a workspace's landing page, which workspace-ui opens when the
workspace is shown, its pinned blocks, which the file tree lists first, and its
templates, which the new-block menu copies (`HostRequest::CopyBlocks`). Which editor
opens a block stays the viewer's choice (`EditorSettings`, no workspace layer), except
that a block may keep its own (`PREFERRED_EDITOR` metadata, from Opens in in Files),
which wins over the viewer's default.

A change syncs by default: it is written to the every-device block. A setting is
kept on this device exactly when this device's profile block sets it; then changes
go there and the other devices' value no longer reaches it. Nothing else is stored to
say so. A screen known only by its connector (no EDID serial) is always this device's
alone. Each field is described once as a `be_block::Setting` (a key, a name, how to
read it from a layer and write it on a layer's own content, and its default), such as
`DisplaySettings::theme_setting()` or `mode_setting(id)`, and `Setting::layer` is the
rule. Every write goes through it: in a plugin through
`block_editor_beui::LayeredSettings` (`this_device(editor)`, or `page(editor)` for a
kind's editor, which acts on this device when opened on the every-device block and on
just its own block when opened on a profile's), which makes the every-device or
profile block when it is missing; in block-app through `display::keep` and `revert`.
Its `LayeredSettingsRow` gives every row a menu (`<id>.menu`) with Keep on this device
(`<id>.keep`, a checked item that copies the value in use, or the default, into the
profile, or deletes it there) and Reset to default (`<id>.reset`, which clears the
layer the setting would be written to), and a "Kept on this device" or "This device
only" note; on a profile's own page its one item is Stop keeping in this profile
(`<id>.reset`, clearing it in that block), with a "Kept in this profile" note. A row whose
value the inspector overrides says so instead (`overridden`, from the `PreferenceSources` host
value, which also gives the system's theme and motion that an unset one shows as its
default). Menu items
carry icons (`ICON_PIN`, `ICON_ROTATE_CCW`, `ICON_PIN_OFF`). `ChangesToast` shows
one toast per visit to a page naming what now applies on every device ("Theme, scale
and 1 more now apply on your devices") with Keep on this device only, which moves those
values into the profile and puts the every-device ones back, and Undo.

A device's class is known before its profile is made: block-app's
`root_settings::device_class` says E-paper tablet when the session found an e-paper
panel, Phone on Android, and Desktop otherwise; Tablet is only ever picked by hand.
A class may have defaults, kept on the device: each settings type lists them in one
place (`DeviceClass::display_defaults`, today only the E-paper tablet's E-paper theme,
instant motion, 150% scale and rotate button; never when the screens go off, nor a rotation).
`ensure_profile` writes a new profile's class and its defaults into the profile's own
blocks, and when there are any the host asks the shell to open the profile. Its page
(the settings plugin's `SettingsProfileView`, `app/setup.rs`) leads with the name and
"Kind of device", and lists in "Where this device differs" each default it still keeps
that differs from every device's value, with "Use my usual settings", which deletes
those from the profile. Nothing is stored to say a profile was set up: while the
profile is this device's and that list is not empty, the page opens as "Set up this
device", with Done (which only closes the setup for this visit) in place of the rest of
the page. Picking another kind writes that kind's defaults and deletes the old kind's
that are still at their default values (`DeviceClass::display_edits`).

The workspace's Settings tab opens the account's `Settings` block on this device's
settings: Display, Input and Default editors open the every-device blocks (making any
that is missing), and Devices and profiles (`settings.devices`) lists the settings
profiles, each of which opens on its own blocks. Above that list, a table (settings plugin
`app/devices.rs`) puts the display and input settings down the side and All devices and
each profile across, with only the rows some profile keeps: a kept cell shows its value,
"kept since setup" while it still holds its class default (`ClassDefault::holds`) and
"kept on device" otherwise; any other cell says "same". It reads each profile's own blocks.
The hotbar is not merged:
the logic editors use the topmost layer that has one, and make one for every
device when there is none yet (`RootSetting::ensure`).

A session is what a person has open, and is separate from the settings, so
starting a new one keeps them. It is a view block (below) of a shell editor:
workspace-ui (`WORKSPACE_EDITOR`), or linux-desktop (`LINUX_DESKTOP_EDITOR`) for
a client run as the desktop with `block-app --session`. `Settings::sessions`
lists every session, and `Settings::session(shell, client)` is the one this
client uses for that shell, so the desktop and the app each keep their own while
sharing the device's settings profile. The host opens the shell on that session,
creating one named after the device when the client has none, and reopens the
shell when it changes. block-app's sessions screen (the organization list's
dropdown, or Sessions and profiles in the app menu) picks the device's settings
profile and its session, or starts a new one of either. A shell keeps its dock
layout and recents in the session's state; every tab, and the file tree, is a
view block child of the session that the layout references, naming the editor
the tab shows it in, and closing a tab detaches its view. A tab on a block that is
itself a view block (a session) uses it as its view and opens the editor it
names, so a workspace-ui session opened on the desktop keeps its tabs in its own
session and closing it leaves that session alone.

### View state

What a person has open is blocks too, view blocks. A view block is a block
whose defined metadata names the editor that draws it in `VIEW_EDITOR` (the `id`
its manifest gives it, never a block type): whoever makes one sets it with
`BlockMetadata::view(editor, name)`, and that key, a host concept, is all
anyone reads to know which editor opens the view (the host's
`RootSettings::session_editor`, block-shell's `editor_of`, linux-desktop's
session list) or that a block is a view at all (the host skips `DELETED_FROM` for
one, and a block's places leave out the views that show it). A view made before
the key, which names no editor, opens as a session in the shell asking and as a
tab in its content's default editor. The content is workspace-ui's `EditorView`
type (`workspace_ui_types::EditorView { content, state }`, its id
`be_block::VIEW_TYPE`, which the host names only to create a session): the block
it shows (none for the file tree or workspace-ui itself), and a `LatestMap` of
opaque `ViewState { bytes, refs }` the editor owns. `refs` are the blocks an
entry points at; `references()` is `content` plus every entry's refs, and a
deleted or replaced child is blanked or repointed in place, so the bytes can
index into `refs`.

Both shells are built from `crates/block-shell`, which holds the tabs, their
views, the saved layout, picks, dialogs, host panels and program windows;
workspace-ui lays them out as Files beside a pane of tabs, and linux-desktop
floats every block in a window of its own over an empty main pane, with a bar
along the bottom for the app menu, the sessions to open and the time.

An editor learns its view block from `Open { view_block }`, which a parent sets
with `ChildTarget::viewed_by(view)`. `editor.view_state(key)` and
`editor.set_view_state(key, state)` read and write it; writes skip the block's
editability, since view state is the viewer's own. An editor on no content (the
file tree, workspace-ui) is opened on its own view block: its manifest names no
`edits`, so it is nobody's built-in editor and Open with never offers it, and the
host opens it only when a parent names it for a block it places as its own view
(`ChildTarget::viewed_by` the block itself), as the host does the shell on its
session.
`be_block::profile` holds the ids of the shells and the file tree.

### What to keep true

The peer writes out what its sessions hold before it goes away: `flush()` seals
and waits, `stop()` does the same and then joins the worker, and the close
handler and every organization change go through one of them, because a session
that is merely dropped loses everything since the last autosave
(`status().unsaved` is what says whether anything would be lost). The host
refuses an `Operate`, a `ReplaceContent` and a graph command for a block the
account may only read: the plugin holds its own edits behind `editable()`, but
the host does not trust it to, so `Instances::editable` asks the mirror's
access. And nothing in the worker runs on a timer. It waits on its command
channel and on the connection's broadcast, with one deadline for the autosave a
dirty session is owed, so an organization nobody is editing costs nothing;
`status().wakes` counts the times it woke, and a test asserts that an idle peer
never does.

A connection that drops does not take the peer with it. `Connection::closed` is
a watch the worker waits on beside its commands - the event broadcast outlives
the socket, because the connection holds a sender of its own, so that is not the
signal - and losing it leaves the last content on screen, backs off, connects
again, reloads the graph and rejoins every block that was open. While it is
down the worker keeps serving from the same `Peer` (the session loop holds a
`Line`, `Up` with the connection's events or `Down` with the next attempt as a
future), and every open replica is handed a session it owns
(`Replicas::disconnected`), so a follower takes over what it had not confirmed
and seals it locally. A block this device already knows does not wait long for
its session while connected: if `join_session`, `watch` and `remote_head` take
longer than `EARLY_OPEN` (250 ms on the modules' clock), the replica starts
from the local head as a follower of a placeholder owner (`JOINING_OWNER`, so it
shows the content and holds edits without relaying them) and the join goes on
in a spawned task (`Io::Joined`). When it lands the host opens the replica again
with the real client and session, replaying the editor messages and host
`Change` requests since it was clean (as reconnecting and restarting a module
do), resending requests it had not answered, then the session changes, relays
and heads heard meanwhile, and moves it to the remote head; a close asked for
meanwhile waits for that. A seal asked of a
joining replica with unconfirmed edits first hands it a session it owns, so a
flush or a stop never waits for the join, and a join that fails for a reason
other than the connection leaves it on that session. A block opened while down opens from its local head with
`OFFLINE_CLIENT` owning a made-up session (`Open::made_up`, set also while
joining); the host answers `Publish` with
`Peer::seal_encoded` (a local commit in the outbox) and reports it as the new
head, `Resume` treats the local head as the remote one, and heartbeats, claims
and relays are skipped. A publish whose connection dies after the commit was
sealed keeps the commit the same way. On each connection the worker first
drains the outbox (`Peer::drain`), merges every `Diverged` block with
`Replicas::merge_diverged` (a three-way merge by the type module, published
with the remote head expected and the local commit as an extra parent; a
conflicted merge reaches `Host::conflict`), then reloads the graph and rejoins
the open blocks, whose `Moved` to the merged head fast-forwards them. The wait
before each attempt doubles from 0.5 s to at most 30 s with each failure in a
row, randomly shortened by up to half so that clients a server dropped together
do not come back together (`reconnect_delay`), and starts over once a connection
succeeds. `be::retry_now()` cuts the wait short: the app calls it whenever its
window gains focus, and it is the hook for network-change and foreground
events. `be::stop` never waits for a connection: stopping while connecting or
backing off returns at once, sealing what the open replicas hold into local
commits first.

`be::status()` reports `connected`, `syncing` (the outbox is being drained),
`waiting` (outbox operations: local commits and queued graph changes not yet on
the server), `unsent` (blocks set aside in the unsent area), `conflicts` and
`conflicted` (blocks a merge left conflicting), `kept` (outbox operations, safe
on this device), `connecting` (no connection attempt has resolved yet), and
`unsaved`: replicas with unsealed edits, queued commands and imports not yet in
the outbox, which is what closing or switching would lose. The close and
organization-switch guards flush and then ask only while `unsaved` is above
zero; what waits in the outbox is kept and sent later. A type module's error
(`Host::report` with its block) clears once that block saves again.

### Reaching the server

The app talks to be-server directly. `crates/block-app/src/accounts.rs` is the
account client: it connects to `{server}/api/be` (be-server accepts the
websocket on any path, so the suffix only matters behind a proxy that routes on
it), registers, logs in and out, lists and creates organizations, and invites and
joins with invite links. What comes back is an account id and a token, and the
app's peer connects with `Credentials::Token` for the organization it opens.

### Keys

Every block is sealed with keys of its own, wrapped by the keys of the scope it
lies in, and the server holds only wrapped keys. `crates/be-keys` holds the
account-level cryptography, `be_store::Keyring` the content keys, and
`be_client::KeyBook` (each `Peer` has one) the rows and their unwrapping.

- **Sealed bytes name their key.** Everything sealed for content - chunks,
  commits, manifests, block metadata, session traffic, wrapped keys - is
  `b"be3K"`, the 16-byte key id, a 12-byte nonce derived from the key and the
  plaintext (so identical content under one key still deduplicates), then the
  ChaCha20Poly1305 ciphertext. `Keyring::open` looks the id up; an unknown id is
  `StoreError::MissingKey`, a missing tag `Corrupt`. Every `Vault` write names
  the key it seals with.
- **Data keys (DEKs).** Each block has its own: random, with a random id, made
  when the block is created (`CreateBlock` carries it, and the server keeps the
  block's current one in `blocks.key_id`, `BlockSummary::key`). A block keeps
  every DEK it ever had (epochs), so its history stays readable.
- **Scope keys (KEKs).** A scope root is a block at the root (a workspace,
  settings), a detached block, or a block with grants of its own;
  `BlockSummary::scope` is a block's nearest one (`BlockGraph::scope`). A DEK is
  wrapped under its scope's current KEK, a scope root's KEK under its parent
  scope's KEK, and a KEK is sealed to each account that may open it with that
  account's identity key (its recovery public key). A new root block's KEK is
  sealed to its author.
- **Who sealed a key.** A key sealed to an account names its sender and is
  sealed by it (`RecoverySecret::seal_for`/`open_from`: an ephemeral X25519
  key and the sender's identity key both agree with the recipient's, and HKDF
  binds both public keys, both account ids and the key's id). A device takes
  such a key only from itself or a contact it verified (`KeyBook::contact`);
  any other row, including one sealed to its public key by someone who only
  knows it (the server), is ignored and nothing is said about it. A key the
  device made, one sealed to it that way, or one wrapped under such a key is
  *trusted* (`KeyBook::trusted`); a key opened only through the escrow key
  (below) is not, nor one the server sealed itself (it does so only for the
  organization settings' scope key, whose rows it labels and so could label
  any key that way), nor any key wrapped under one of those. A device reads with any key it holds but seals content,
  wraps or seals a key, or hands a key over (`seal_to`, `wrap`, `escrow_wrap`,
  `Peer::sealing_key`, `Peer::offer`) only with trusted ones, so a key the
  server made up never protects anything new. A rotation by such a device
  leaves the data keys it does not trust where they are, still readable under
  the old scope key by whoever holds it.
- **On the server.** `keys` (id, block, kind `data`, `scope` or `escrow`,
  epoch) and `wrapped_keys` (key, `wrapped_by`: a key id or `account:{uuid}`,
  the sender, which the server sets to the account storing the row, sealed). `GetKeys` serves the key rows of the blocks the account can view
  (all of them, or the named blocks and their ancestors) and the wrapped rows
  wrapped by its own account or by a key it is served; nothing for a block it no
  longer sees, and nothing sealed to another account. `PutKeys`, `CreateBlock`
  and `SetParent` store rows (`keys::store_keys`): only for blocks the caller
  may edit (for a move, the moved subtree), never replacing a sealed row, and a
  key wrapped under a new scope key loses its wraps under the old ones (escrow
  aside). Collecting a block drops its keys.
- **On the client.** `Peer::signed_in` loads every row it is served (and keeps it
  in the refs database, so `Peer::cached` unwraps the same keys offline) and unwraps
  what it can (`KeyBook::absorb`: its own account's rows with the identity
  secret, then key under key). A read that meets an unknown key fetches the rows
  again once, and otherwise fails with `ClientError::Locked`; `Peer::learn`
  fetches the rows of blocks that appear later (the worker calls it for every
  summary it is told about). A write seals with `Peer::sealing_key`, the
  block's current DEK.
- **Moving** (`Peer::set_parent`) into another scope wraps the moved blocks'
  DEKs, old epochs too, under the destination's KEK, gives each a fresh DEK,
  and wraps any scope root inside it under that KEK, all in the `SetParent`
  request. A move within a scope, or to Recently Deleted, changes no keys.
  `Peer::share` grants and makes the block a scope root of its own (a KEK, its
  subtree rewrapped under it) sealed to the grantee's identity key, which is
  only ever a contact's verified key (below). The server refuses to store a
  key sealed to any account but the caller's own and its contacts'.
- **Rotation.** When an account stops being able to view a block (a grant
  revoked or left, the block moved out from under what it could view, a
  contact removed, a member removed), the server records
  the block in `rotations`, drops that account's wraps of the block's keys and
  ends its sessions on what it can no longer see (a client must be in a
  session to speak in it). `GetKeyWork` tells a client what it can do about
  keys: the rotations on blocks it may edit, scope keys (and the escrow key)
  not yet sealed to someone who should have them (`unsealed`), and scope keys
  not yet wrapped under the escrow key (`unescrowed`). `Peer::tend` does all of
  it; the app's worker calls it on connect, on `ContactsChanged`, on
  `KeyWorkChanged` (sent to a block's audience when a revocation or move
  records a rotation) and after its own changes. `Peer::rotate` walks the block's
  subtree and sends one `Rotate`: a new epoch of every scope key in it, each
  wrapped under its parent scope's new (or, above the block, current) key and
  sealed only to the rotating account, every data key of every block (old epochs
  too) rewrapped under its scope's new key, and a new data key per block; `tend`
  then seals the new scope keys through `unsealed` like any other. The
  server takes it only whole: every block below the root a new data key, every
  scope root a new scope key, in one transaction, and drops the account wraps
  of the old scope keys for anyone who can no longer view them. Content is
  never sealed again, except that the worker has the owner of a live session
  on a block whose key changed seal it once more (`Session::rekey`), so the
  head is under the new key. A rotation nobody could do yet (the one who
  left cannot edit, or lacks a contact's key) waits in `rotations` until a
  client that may edit the block tends; grantees it could not seal to are
  listed as `unsealed` for any device that holds the key and the contact.
  Whoever should hold a scope key is `keys::sealed_to`: its grantees, the author
  of a root block, for a kind open inside everyone who views a block inside its
  owner, and for the organization settings the server and every administrator.
  A wrap the server sealed counts as unsealed, so an administrator still gets a
  trusted copy from another. A kind open inside is also offered to anyone who
  views it, for their own contacts among those viewers (`PutKeys` takes that
  seal from a viewer), since its owner's editors may have none of them as
  contacts; closing it rotates its key for everyone who no longer views it.
- **Escrow** (company organizations). The escrow key is a 32-byte secret
  (`be_keys::EscrowSecret`) that is both an X25519 key and, through HKDF, an
  Ed25519 signing key. It is a key row of kind `escrow` (its block is the
  organization id; `keys.public`, `signing` and `signature` hold
  `EscrowKey`), sealed to each administrator by another. The first
  administrator device to open the organization makes it (`CreateEscrow`),
  and `mint_scope` seals every new scope key to its public half too, so
  administrators open every scope key and with them everything. Only
  administrators are served the escrow rows. Promoting someone leaves the
  escrow key unsealed to them until an administrator who has them as a
  contact tends; demoting or removing an administrator records an escrow
  rotation, which an administrator holding the current escrow secret does
  with a new `CreateEscrow`: the next epoch, signed by the current one over
  `EscrowKey::chained` (organization, previous id, new id, epoch, both public
  halves), sealed to the remaining administrators, with every scope key it
  holds wrapped under it and the old escrow wraps of those keys dropped.
- **Pinning the escrow key.** `OrganizationOpened` and `KeyWork` carry the
  whole chain, which the server could make up, so a member wraps only under an
  escrow key it pinned from somewhere the server cannot forge: its own (the
  creator), the invitation (the inviter's verified key, sealed with the invite
  code, `InviteCode::seal_escrow`), an agent's maker, or a secret it holds
  because an administrator contact sealed it to it. From the pinned key it
  follows the chain one signature at a time (`keys::verify_escrow`) and pins
  the newest it verified (`PinEscrow`: sealed for itself, so every device of
  the account has it; `OwnKind::Escrow` rows are resealed with a new phrase
  like contacts). A newer key it cannot verify is not used: it keeps wrapping
  under the last verified one, and the app menu shows the warning
  (`KeyBook::escrow_warning`); with nothing pinned it escrows nothing.
- **Identity.** Each account has a twelve-word recovery phrase, shown once and
  checked by typing three of its words. The phrase derives an X25519 key pair;
  the server stores only the public half (`SetRecoveryKey`, `GetRecoveryKey`),
  and a `Peer` sets it from its secret when the account has none. A device keeps
  the secret itself (`AppStateStore::identity`), and an account whose device
  lacks it, or holds one the server's public key does not match, opens on the
  unlock screen (`crates/block-app/src/keys.rs`, `ui/keys.rs`): the phrase, or
  a pairing code. A new phrase (the organization menu's New recovery phrase)
  must reseal every row sealed to the account by itself or a verified contact
  (others are dropped), and everything the account sealed for itself (contact
  rows, link codes and pinned escrow keys, `OwnSealed`), which the server
  checks, so only a device holding the old secret can make one; the account's
  other devices then ask to be unlocked again. Contacts keep working across
  it: the new phrase sends each verified contact a key change
  (`be_client::contacts::new_identity`, `RecoverySecret::seal_key_change`: the
  account id and new public key, sealed from the old secret to the contact),
  which the server keeps in `key_changes` and hands out with `ListContacts`.
  A contact's device follows the changes one at a time from the key it holds
  (`contacts::contacts`), so a change not sealed by that key is ignored; when
  it reaches a newer key it seals the keys that account had sealed to it again
  for itself (`ResealKeys`, since they were sealed by the old key) and then
  replaces its contact row (`PutContact`).
- **Inviting.** Joining an organization takes an invite link, the only way in,
  and hands over no key: keys come with what is shared. The inviting device
  makes a random invite code (`be_keys::InviteCode`, 200 bits) and names the
  invitation by an id derived from it (`InviteCode::lookup`), so the link only
  needs the code: `{server}/#invite/{CODE}` (`invite_link.rs`). The invitation
  carries the inviter's contact card and the organization's escrow key as the
  inviter verified it, both sealed with the code, and accepting it sends the
  invitee's card back, exactly like a contact link (below), and pins the
  escrow key. The dialog shows
  the link once. The invitee pastes it on the Organizations screen, or opens it
  in the web app, which takes it from the address bar on load and replaces it
  with `#`. The server never sees the part of the link after `#`.
- **Adding a device.** The new device shows an eight-character code. The user
  types it on a device of the same account that has any organization open, and
  the two run SPAKE2 keyed by the code and the account id over the server's
  relay (`Pair`, delivered as `Paired` to the account's other connections). The
  open device seals the identity secret under the agreed key, so the server
  would have to guess the code to read or replace it. It gets one guess,
  because a wrong code ends the request.

### Contacts

Nobody reaches a stranger from inside the app: every sharing relationship
starts with a link one person passes to another outside it. A contact is what
that exchange leaves behind, and a device seals a key only to a contact's
public key it verified this way, never to one the server hands it.

- **A contact card** (`be_keys::ContactCard`) is an account id, its identity
  public key, a display name and an email. A contact link is an `InviteCode`
  like an invite's, `{server}/#contact/{CODE}`; the card is sealed with the
  code (`ContactCard::seal`, keyed by the code and by the side, owner or
  accepter, so one cannot stand in for the other), and the server, which never
  sees the code, can neither read nor replace it. What an account keeps for
  itself (its contacts' cards, its links' codes) is sealed with a key derived
  from its identity secret (`RecoverySecret::seal_own`): sealing to the public
  key would let anyone, the server included, make a row the owner would trust.
- **The exchange** (`be_client::contacts`). The maker posts
  `CreateContactLink` under `InviteCode::lookup`: its card sealed with the code,
  and the code sealed for itself, so any of its devices can finish. The other
  person reads it (`GetContactLink`), opens the card, which only the code
  opens, and on "Connect with NAME (EMAIL)?" accepts (`AcceptContactLink`),
  posting its own card sealed with the code and its row for the maker. The
  server records both `contacts` rows (the maker's empty), marks the link used
  and sends `ContactsChanged` to both accounts. The maker's device then lists
  its links (`ListContactLinks`), opens the accepter's card with the code,
  writes its row (`PutContact`) and deletes the link (`finish_links`; the app's
  worker does it on connect and on `ContactsChanged`). Organization invites
  finish the same way: accepting one leaves a used link behind for the inviter.
- **A pending grant.** A link may carry a block, an access level and that
  block's own scope key sealed with the code and bound to the key's id
  (`Peer::offer` makes the block a scope root first). Accepting it has the
  accepter seal the key for itself, and the server
  applies the grant and stores that wrap at once, so the share works while the
  maker is offline; when it finishes, it seals the scope again
  (`Peer::seal_share`), which also covers blocks made inside it meanwhile. The
  grant is applied only if the accepter is a member of that organization. A link whose grant is in another
  organization than the one the maker's device has open stays until a device
  of the maker opens that one. A grant to a contact whose key the granter has
  not verified yet works on the server at once, and the key is sealed to them
  (`unsealed`, above) when the granter's device verifies it; the share dialog
  says so meanwhile.
- **On the server** `contact_links` (lookup id, owner, card, code, the pending
  grant, the accepter and its card) and `contacts` (account, contact, the
  contact's card sealed for the account). Contacts are per account, across
  organizations; a grant needs both to be members of the block's organization
  too. `RemoveContact` deletes both rows and, in every organization they share,
  revokes each one's grants on blocks the other may edit, which records their
  rotations.
- **In the app** the Contacts dialog (app menu) lists contacts, removes them,
  makes a link shown once with a copy button, and opens a pasted one; the web
  app takes `#contact/` from the address bar like `#invite/`. The worker keeps
  `Shared::contacts` (each with its verified key, or none until the exchange
  finishes) and seals a share only to those keys. The share dialog lists
  contacts and current grantees, lists them again when the `Contacts` host
  value changes and after each grant it makes, and its "Invite someone new"
  makes a link carrying a pending grant of the block (`HostRequest::ContactLink`). Invite
  member and Agents show only in a company organization, and Members (list,
  change a role, remove, invite) only to its administrators.

### What the server can and cannot do

The server stores and relays everything, so it can withhold, delay, replay
or delete rows, refuse requests, and see who shares with whom, block shapes,
sizes and timing. It cannot read content or metadata, and it cannot choose a
key a device will seal with:

- every key sealed to an account is authenticated by its sender, and a device
  takes it only from itself or a contact it verified through a link;
- a contact's key is replaced only by a key change sealed with the key the
  device verified;
- contact cards, link codes, pending grants and the escrow key in an
  invitation are sealed with codes the server never sees;
- a key wrapped under another key is AEAD under that key, and a device wraps
  only under keys it trusts;
- an escrow key is used only when pinned or signed down a chain from a pinned
  one.

What it still decides, and a device trusts it for:

- which of the keys a device holds is a block's current data key and which
  epoch is newest: a server working with someone who lost access, or with an
  administrator who was demoted (who knows the old escrow signing key), can
  keep a device writing under a key that person knows;
- grants, membership and roles: a device seals a scope key to any verified
  contact the server lists as granted on it (`unsealed`, rotations), and the
  escrow secret to any verified contact it lists as an administrator;
- which public key belongs to a non-contact (never used for sealing), and
  the rows of keys opened only through escrow, which administrators read but
  never write under;
- which blocks an account knows exist: it sends the id and type of a block
  an account may view references, and could send those of any other.

### Presence

Presence is what a peer shows the others while it is there - a cursor, a
selection - and it is never saved. `SessionMessage::Presence { kind, value }`
carries one kind of it to everyone in the session, sealed like every other
session message. A replica sends a value (or takes it away with `None`) only
when it changed, keeps what the other peers show keyed by their `ClientId` and
the kind, and never shows a peer its own.
When `SessionChanged` names a participant it has not seen, a peer shows that
participant what it is showing, so a late joiner sees everyone at once; when a
participant leaves, what it showed is dropped.

A replica sends each editor attached to it the block's peers as
`ToEditor::Presence` in a `Replica` frame when they change. A plugin shows presence with
`FromEditor::ShowPresence`, which the replica sends to the other participants.
`editor.peers` and `editor.on_peer_message` send `Resend` once, so an editor that
never reads its content is still attached and hears its peers. On the
plugin side that is `editor.show(Some(&value))` and `editor.peers::<P>()`, a
signal of every peer's value of one `PresenceKind` (`be_block::presence`). The
framework shows a `UserActive` value of its own, with a colour picked by
`pick_free_color` against the colours already showing, whenever the host says the
block is visible. The text editor's cursors and the canvas's pointers and
selections work the same way, and each carries a colour of its own picked the
same way rather than joining it to the peer's `UserActive` entry.

### Peer messages

Presence is state; a peer message is a message. `SessionMessage::Channel {
channel, payload }` goes to one participant of a block's session or to all of
them, sealed with the block's key like every other session message, so the
server relays what it cannot read. A channel is a uuid naming what the payload
means. A participant hears what each of the others sent it in the order they
sent it, and never hears itself. Nothing is kept: a participant that was not in the session never
hears it.

A replica hands each editor attached to it the messages that arrived since it
attached, as `ToEditor::PeerMessages`; an instance sends with
`FromEditor::SendToPeers`. On the plugin side that is
`editor.send_to_peers(to, &message)` and `editor.on_peer_message(|from, message|
..)` over a `ChannelKind` (`be_block::presence`), which names the channel and
encodes with postcard. `to` is a client id from presence. The terminal block is
built on it (guides/terminal.md).

### Editors with their own model

The text editor cannot hand its state to a projection: `text_editor_core`
wants a `Document` that hands out positions, so a cursor stays on its
character while other people type. `block_editor_beui::live_text::LiveText`
is that document for any `Text` or `RichText` field of any block, and it is
the only one: the text editor and anything else that edits text in place
(a canvas text shape or label) mount the editor on one. Each projection
has one log of the edits it applied, so only one `LiveText` per projection
syncs at a time: the canvas syncs the text being edited and has the others
take in their whole field when it changes. A `TextField` names the
object and field it edits, and optionally a language and indentation the
block stores beside it (`text_block::document::TextBody` does that for a text
block); `FieldOf::rich` and `FieldOf::plain` name a field and nothing else. It
keeps its own copy of the field's sequence and marks, turns each local edit
into `Change::Text` and each format into `Change::Mark` (a plain `Text`
takes no formats), and `LiveText::sync`, called on every change of the
document and of the projection (the projection must `record()`, and
`stop_recording()` once nothing reads its log), sends them,
asks the host to undo or redo, applies the edits others made to the field in
place, and takes in the whole field otherwise, which is a no-op when nothing
in it changed. Undo is the host's (`Document::step`), so it leaves other
people's typing and formatting alone.
A caret in a live text travels as presence through
`block_editor_beui::SharedCaret` (its anchor and focus as `Position`s, which
every copy of the field resolves to the same place): `SharedCaret::of(area)`
reads it, and `remote_carets(area, carets)` turns peers' carets into the
`remote_cursors` a `TextArea` draws. The text editor's `TextCursor` and the
canvas's `CanvasCursor::caret` (which also names the entity) both carry one.

Dynamic artifacts reach content from the artifact session through the
host: `EditorHost::content_of` gives a regeneration a projection of its source,
and `EditorHost::replace_content` writes the result. Compiled logic and the
pixel art export regenerate this way.

## What is not built yet

- A commit is made from each block's head after this peer seals what it owns,
  so an edit another peer is holding in a session it has not sealed yet is left
  out. A seal barrier would close that: the server asks the owner of every
  session in the checkout to seal, and the commit waits for them.
- Conflicts are resolved a block at a time by taking one side; nothing merges
  inside a block by hand yet, and the three sides are linked, not shown side by
  side.
- A repository never lets go of history: deleting a branch frees nothing until
  the repository block itself is collected. Freeing it wants the server to
  count holds per object, with a commit holding its parents and its tree.
- An upstream is another repository in the same organization. Across organizations
  the keys differ, so a pull would have to reseal every object.
- A snapshot tree reuses the manifests of the blocks it holds, sealed with their
  own data keys, so reading a repository needs the keys of every block it took
  a snapshot of.
- Moving a block into or out of a checkout keeps the ids its content stores, so
  what they name changes with the scope it lands in.
- Nothing vouches for who wrote an item. The deterministic game stores the
  account behind each move as a field the sender fills in, so a player can move
  as someone else. Signing operations would not be enough, because the session
  owner sequences and relays everything: the items a block stores have to carry
  their own signatures, checked by whoever reads them. That wants doing when a
  chat block makes it matter.
- Nothing reads content but the editor holding it: there is no preview built
  from content outside an editor and no search over it. Server-side search is
  impossible under end-to-end encryption, so it has to become client-side over
  the local cache, or a client-built encrypted index.
- Deleted blocks are never collected by the app. be-server's
  `CollectDetached` (`Peer::collect_detached`) removes every deleted block the
  caller may edit (`BlockGraph::detached`: each deleted root's subtree), with
  its objects, but nothing calls it. When something does, it will remove a
  deleted block that is still linked and leave the link dangling, so the
  content that links it has to cope with a block that is gone, and it will race
  a new child its parent has not listed yet.
- A tab that closes leaves whatever a session held since its last autosave
  behind: `flush()` cannot wait there. A reload or close asks first while
  anything is unsaved (`beforeunload`), an edit on its way to its module
  included (`Replicas::counts`), and the worker seals meanwhile.
- While a request other than a listing waits on a server that has stopped
  answering (a summary refresh, `ReadHead`, versions), the worker's
  loop waits with it, and a command queued behind it (an open, an edit) waits
  until the connection is declared lost.
- A move into another scope lists the moved subtree one level at a time
  (`ListChildren`), and the old data keys of a block this device cannot open
  are not rewrapped, so that block's history stays closed to its new scope.
- Sessions relay through the server. Direct peer connections are a latency
  optimisation on the same protocol and can come later.
- The server learns the shape of the graph, object sizes and timings. It never
  learns content. If graph privacy matters later, block ids can be blinded per
  organization, but the server still needs some graph to do access inheritance.
- Retention is driven by the client calling `Peer::prune`. Nothing schedules it.
- An empty session is remembered while it carries a `clean_at`, which is bounded
  by the number of blocks that have been edited live. If that grows, evict by
  age rather than dropping the marker.
