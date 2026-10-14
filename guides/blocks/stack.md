# The block stack

Every block lives in the `be-*` crates. Read this guide to work on the stack itself: storage,
commits, sessions, the graph, the server, the client peer and version control. To add a content
type, read `guides/blocks/adding_a_block.md`; for how block-app hosts content and hands it to
plugins, read `guides/blocks/in_the_app.md`; for what must stay readable forever,
`guides/blocks/frozen_formats.md`.

The app is a peer of its own (`crates/block-app/src/be.rs` and `be/`), the server is
`be-server`, and every editor keeps its content in these crates.

## Running a server

```
./scripts/buck run //crates/be-server:bin -- --add-account you@example.com "You" hunter2hunter2 Workspace
./scripts/buck run //crates/be-server:bin -- --addr 127.0.0.1:9090 --data-dir be-server-data
```

| Flag | Meaning |
|---|---|
| `--addr`, `--address` | Listen address. Default `127.0.0.1:9090`. |
| `--data-dir` | Data directory. Default `be-server-data`. |
| `--add-account EMAIL NAME WORKSPACE` | Provision an account and its workspace; the password is read from standard input. |
| `--allow-registration` | Accept `Register`. Without it, registration is refused. |
| `--allow-login` | Accept `Login`. Without it, only tokens of devices already signed in work. |
| `--build-id-file` | Where to read the build id from (default: the `build-id` file beside the executable). |

You rarely need a server of your own:

- The desktop app embeds be-server (`crates/block-app/src/platform/native.rs`) for a local
  account, on an ephemeral port with a data directory under the app's, and signs in to it like
  any other server. The embedded server and the dev targets allow registration.
- An account on another server connects to that server's URL. The web build always does:
  `./scripts/buck run //crates/block-app:web` starts be-server beside it, closed to new accounts.
- Tests start their own server on an ephemeral port: `Harness` in `crates/be-client/src/tests.rs`
  and `crates/be-server/src/tests.rs`. The app's tests (`Harness` in
  `crates/block-app/src/be/tests.rs`) go through the embedded server, register an account,
  create a workspace and start the app's peer against it the way the app does.

`guides/hosting.md` covers running be-server on a VPS.

## Goals

1. Durable state is bytes. A block's persistent form is a byte string, stored as an ordered list
   of chunks. Backups are a commit chain over those bytes.
2. Live editing converges through a session owner, not a CRDT. A type may still be a CRDT where
   that is genuinely better, but nothing depends on it.
3. Offline divergence resolves by three-way merge over the persistent bytes, so a wholesale
   rewrite conflicts visibly instead of interleaving.
4. Partial reads. A ten gigabyte video streams; it is not loaded to be shown.
5. The server never sees content. It holds objects, refs, a graph and accounts, and understands
   only the last three.
6. Reclamation is reference counting, not tracing.

## Rules not to break

Each of these keeps a goal true. Do not trade one away without replacing what it protects.

- Keep durable form, live-edit operation, undo history, reference graph and child manipulation
  separate. One trait or flag that governs live merge, offline merge and the wire protocol
  together couples them all.
- No append-only operation log as durable state. It never compacts, and compaction needs to
  understand the operations, which the server, holding only ciphertext, cannot.
- Record references once per commit, from the sealed state. Never diff the reference set on every
  edit or rewrite a workspace's reference rows on every update.
- Unsent work is persisted locally, not held only in memory.
- Bulk data travels out of band, as chunks, never encoded into an operation or a JSON snapshot.
- Version control is built on the substrate (be-store objects, be-commit commits), not as block
  types beneath which storage is a guest.
- Liveness comes from the parent chain only; no tracing collector (see be-graph).

## The layers

| Crate | Holds | Runs on |
|---|---|---|
| `be-store` | objects, chunking, encryption | client + server |
| `be-commit` | commit DAG, retention, merge | client + server |
| `be-session` | ownership lease, sequencer, resume | client + server |
| `be-graph` | parents, edges, access, refcounts | server |
| `be-protocol` | the wire format | client + server |
| `be-model` | objects with ids, merge, undo, derive | client |
| `be-block` | content traits and content types | client |
| `be-vcs` | snapshot trees, diff, tree merge | client |
| `be-client` | a peer: local store, blocks, `Live` | client |
| `be-server` | the always-online peer | server |

The server is one more peer that never decrypts, which is why most crates are shared. A client
is a peer with a local store and a UI; a server is a peer with a big store, no UI and the session
registry.

## be-store

Content-addressed, encrypted object storage.

- `Hash` is the SHA-256 of the ciphertext, so a server can verify what it holds without reading
  it. A hash is a CBOR byte string.
- `chunker::split` is a gear-hash content-defined chunker. An edit in the middle of a large file
  re-uploads one chunk; the chunks either side resynchronise.
- `Vault::seal` derives its nonce from the key and the plaintext, so identical content
  deduplicates under one key and shares nothing across keys. Do not switch to a random nonce
  without replacing the deduplication story too.
- `Manifest` is `{content_type, length, chunks}`. `Vault::read_range` and `chunks_in_range` are
  the streaming primitives.
- `ObjectStore` is `MemoryStore` or `FileStore`. It is synchronous on purpose: network fetching
  is explicit (`Peer::fetch`), so nothing blocks inside it.
- `Vault::put_value` stores a value (a commit, a version-control tree) as a CBOR map by field
  name (`encode_value`). A later build can add a field with `#[serde(default)]` without a
  migration. Commits are encrypted, so only a client could rewrite one, and rewriting changes its
  hash and every id that names it: an encrypted format is read as it was written, forever.

## be-commit

- `Commit` is `{parents, manifest, author, time, kind, references, metadata}`, stored as an
  encrypted object whose hash is its `CommitId`. Several parents means a merge.
- `Publish` also tells the server the parents in plaintext, so the server knows every block's
  history graph without reading a commit.
- `references` is the edge set derived from the sealed state, recorded once per commit, so the
  reference graph always describes content that was actually stored.
- `CommitStore` walks the DAG and tolerates pruned history: a missing commit truncates a chain
  instead of breaking it, so `first_parent_chain`, `ancestry` and `common_ancestor` keep working
  after retention runs.
- `retention::plan` thins history. It pins bookmarks, keeps everything inside `keep_all_within`,
  then thins by widening age buckets, preferring in each bucket the commit with the longest quiet
  interval after it: a state left alone for an hour outranks the keystroke bursts around it.
  Retention is driven by the client calling `Peer::prune`; nothing schedules it yet.
- `merge` is diff3. `merge_slices` walks the base indices both sides left equal; regions between
  those anchors take whichever side changed, or conflict when both did. `merge_lines`,
  `merge_map` and `render_conflicts` are built on it.

## be-session

- `Lease` is ownership. The first participant owns the session. A claim succeeds only against
  the generation the claimant last saw, and only when the lease expired or the owner is gone.
  Generation numbers stop a stale claim from stomping a newer owner.
- `clean_at` is the commit the owner last reported durable. It outlives the session going empty
  (`sessions::forgettable`), so a peer taking over an abandoned session can tell whether anything
  was unpublished. An empty session is remembered while it carries a `clean_at`; that is bounded
  by the number of blocks edited live. If it grows, evict by age rather than dropping the marker.
- `resume` decides whether a returning peer merges, from history, not from having been away: an
  ancestor head fast-forwards, a descendant head publishes, only genuine divergence merges.
- `Sequencer` and `Follower` are the two ends of the ordering. Operations carry an `OpId`, so
  work submitted before a connection dropped is deduplicated on the way back in, not applied
  twice.

## be-graph

Liveness is hierarchical. A block is live when its parent chain reaches `BlockParent::Root`. A
block with no parent is pending deletion together with its subtree, even when something else
still references it. References are for navigation and access, never for liveness.

That makes reclamation reference counting: a block holds its commits, a commit holds its chunk
hashes, and `ObjectRefs` deletes an object when its last holder releases it. There is no cycle
to trace and no mark and sweep. Do not reintroduce one.

## be-server

SQLite for metadata, a `FileStore` for objects, and a websocket carrying postcard frames.

### Connecting

- A connection starts with a greeting outside the protocol: each side sends `be_protocol::hello`
  with its build id (`build_id::current()`, read from the `build-id` file beside the executable,
  or `--build-id-file`). The server closes the connection when the ids differ
  (`ClientError::Incompatible` on the client).
- A program with no build id is an error, not a wildcard: the client fails with
  `ClientError::NoBuild` and the server does not start. A test installs one with
  `build_id::install`.
- Server and clients are upgraded together from one commit, so the protocol is postcard and never
  versioned.
- Where a websocket cannot get through (a proxy refusing the upgrade), `POST .../http` carries the
  same frames (`http.rs`):
  - The first body is the greeting; its answer names a session in a `Be3-Session` header.
  - Each later post with that header carries client frames, answered in order with the frames the
    server has for the client then.
  - A body is frames, each after a four-byte big-endian length.
  - The client sends whatever is queued in one post, and `Peer::fetch` asks for every missing
    object at once.
  - A session is kept like a websocket connection, until idle for the same time, so
    notifications wait for the next post.
- The server never reads `Authorization`; that is left to a gateway in front of it.

### The database

The schema is a list of `CREATE` statements (`schema.rs`). `sqlite_schema::ensure` checks a
database against it on open: it adds a missing table, index, or column that has a default, and
refuses anything else (a column gone, retyped or unknown), so a schema only grows. Values that
may grow (a role, an access level, a parent kind) have no `CHECK`, since SQLite rebuilds a table
to change one. The app's own `app.sqlite3` is checked the same way, and recreated (signing the
device out) when it does not match.

The graph is cached per workspace in memory and dropped on any error, so it reloads from the
database rather than drifting.

### Publishing

- A publish is a compare-and-swap on the head. Against a stale head it comes back `Rejected` with
  the head to merge against, rather than being silently ordered after it.
- A publish writes one head update, the few edge rows the commit declares, and refcounts for the
  chunks it introduced. It does not rewrite the workspace.

### Access

- Access is computed per member (`Visibility` in `blocks.rs`, over `BlockGraph::access_map`).
- A workspace administrator edits everything.
- Anyone else gets `Edit` on what they authored and on whatever they were granted, both inherited
  down through parents.
- A block they can see referencing another gives them `KnowExists` on it, and so does being an
  ancestor of anything they know of, so the path to a shared block is visible without its content.
- `SetAccess` grants; only someone who may edit a block may grant on it.
- Graph announcements are per member: when a block changes, every connected member is sent the
  `BlockChanged` their access lets them see, or `BlockRemoved` when it no longer does.
- An object is served only to a member who can view a block that holds it, by a commit
  (`commit_objects`) or by `HoldObjects`; anything else reads as missing. A merge commit holds its
  extra parents' objects, so a `stash` that was never a head stays readable.

### Accounts

- `Register` (needs `--allow-registration`), `Login` (needs `--allow-login`), `Authenticate` with
  a token, and `Logout`.
- Workspaces are created through the account API. An administrator can `Invite` an email; the
  invitee sees it with `ListInvitations` and answers with `RespondInvitation`.
- Password checks run off the database lock, a few at a time. An email with five failed sign-ins
  in a row is locked out for a time that doubles with each further failure.
- An administrator's `CreateAgent` makes an agent: an editor account with no password, its
  workspace key sealed to a recovery key the caller made for it, and a token, all answered once
  (`guides/tracker.md`).

## be-block: the content traits

A block's content is a set of traits a type opts into (`crates/be-block/src/lib.rs`, abridged):

```rust
trait BlockContent {                    // durable form and the edges it declares
    const CONTENT_TYPE: Uuid;
    const FROZEN: bool = false;
    fn schema() -> String { String::new() }
    fn encode(&self) -> Vec<u8>;
    fn decode(bytes: &[u8]) -> Result<Self, ContentError>;
    fn references(&self) -> Vec<Uuid> { Vec::new() }
    fn references_in(&self, workspace: Uuid) -> Vec<Uuid> { self.references() }
    fn name(&self) -> Option<String> { None }
    fn describe(&self, metadata: &mut BlockMetadata) {}
}
trait LiveEdit: BlockContent {          // optional: what a session carries
    type Op;
    fn apply(&mut self, operation: &Self::Op);
    fn apply_touching(&mut self, operation: &Self::Op, touched: &mut Vec<Touched>);
    fn child_operations(&self, change: ChildChange) -> Option<Vec<Self::Op>>;
    fn rebase(operation: Self::Op, onto: &[Self::Op]) -> Option<Self::Op>;
    fn session_state(&self) -> Vec<u8>;
    // also absorb_operation, encode_operation, decode_operation, adopt_session_state
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

- History and child manipulation are not part of the durable form. A child change is an ordinary
  operation that `LiveEdit::child_operations` answers for.
- Undo is client-local; against a sequencer it emits an inverse operation rather than rewinding
  state. `Undo::step` is taken from the state before an operation, and `revert` and `reapply`
  turn a step into operations against the state as it is now, so an undo leaves alone whatever
  someone else changed since: the calendar undoes a rename without moving an event another peer
  rescheduled.
- `Streamed` types encode as `[u32 header length][header][payload]`, so a reader can take the
  header and then a byte range. Every `Blob` is streamed (`guides/blocks/adding_a_block.md`,
  "Files: Blob").
- `references_in` is `references` for one workspace; the peer calls it when it records a commit's
  references (`Root::references_in` for a document). Text uses it: a URL pasted from another
  workspace is a link, not a reference.

`Document<R>` implements all of these but `Streamed` for any `be_model` root (`model.rs`), which
is how almost every type is built (`guides/blocks/adding_a_block.md`).

### Block URLs

`be_block::block_url` holds the format: `<base>#block/<workspace>/<block>`, where the base is the
URL of the server the workspace is on (the `LinkBase` host value), sub-path and all, or nothing
for the device's own server. Text stores these bytes, so the format is read forever. A reference
is found by its fragment alone, whatever host precedes it, and the link it belongs to starts at
the `http://` or `https://` directly before the `#`.

## be-model: how documents work

`be-model` stores a document as a table of objects with ids, not a tree of values, and every
algorithm is written once against that table. `guides/blocks/adding_a_block.md` covers the field
kinds and the stored form; this is what happens underneath.

### Identity and edits

- Every object in a `List` has an `ObjectId` (the `id` of the `Item` the list reads back as).
  Edits name the object, not a position. An edit to an object that moved into another list still
  lands on it, and `move_into` carries an object and everything under it between lists.
- The derive gives each field a typed constant (`CalendarEvent::TITLE`) that builds the changes an
  `Edit` is made of: `set` (register), `add` (`Count`), `put` (`Map` key), `insert` and
  `move_into` (`List`), and `Change::remove` for any object.

### Live editing

- Edits address objects by id and anchor inserts to a sibling, so they mean the same thing
  whatever the sequencer put before them: there is nothing to rebase.
- A list is a `Sequence` of ids (`Items`) that remembers each object's slot. An insert, or a move
  into a list the object was never in, takes the next offset of the client that made it
  (`local_client`, carried in the change), so every replica gives it the same slot.
- Removing an object leaves a tombstone. An insert anchored after a removed or moved-away object
  lands where it was, and an object that comes back to a list reuses its slot. A move within a
  list keeps the slot; a move to another list leaves a tombstone behind.
- Tombstones and text positions live only in the session. They are never saved; the owner hands
  them to a joining follower in `Snapshot` and to reloading followers in `Sealed`
  (`LiveEdit::session_state`), and they go when the session ends.
- A `Snapshot` carries the state as of the owner's last seal, because the follower opens the
  sealed head and applies the operations since onto it. Adopting state rebuilds a follower's view
  from `confirmed` and its pending edits rather than adopting into it.
- Anything two peers may create at once for the same purpose (a database row past the end, a
  canvas component for a schema, a logic game solution) takes an id derived from what it is for,
  so the second insert is refused and its edits land on the first.

### Offline merge

`Document::merge` matches objects by id across the three versions and merges each field on its
own:

- Registers take whichever side changed; counts add both sides.
- Lists merge their order the way `merge_slices` merges lines, once the items either side moved
  are taken out and put back after the sibling they followed on that side.
- An object moved on one side and edited on the other keeps both.
- You delete what you saw: deleting an object (or map key) the other side edited deletes it and
  counts a conflict.
- What the other side moved or inserted into a removed container survives: a moved object goes
  back to the list it came from (or the first sibling container), a new one brings its container
  back, and either counts a conflict.

### Undo

- `Document::step` records, per change, the change that undoes it and the one that redoes it,
  both taken against the state before the edit.
- A register's undo is conditional (`Change::SetIf`, `Change::PutIf` for a map key): it puts the
  old value back only if nobody changed it since, which is how undo leaves others' edits alone.
- A move's undo puts the object back after what preceded it, even if someone moved it since; its
  redo moves it where the move put it.
- A removed object comes back with everything under it, in its old slot.
- `Change::RemoveIf` removes an object only if it still holds what the remover expected, for
  cleanups like a database dropping a row it emptied.
- Consecutive sets of the same fields absorb into one step.

### Grids

- A paint is a list of cells, each optionally conditional on what the cell held, so a paint's
  undo repaints only cells nobody has painted since, and a burst of paints into one grid undoes as
  one stroke.
- An offline merge takes the bounds like a register, then merges each coordinate on its own, so a
  paint made during a resize lands where it was painted, and a crop's undo repaints what the crop
  dropped.

## be-client: the peer

`Peer` is a local object store plus a connection.

### Transport

- `transport/` has a `Writer` and a `Reader` per platform (tokio and tokio-tungstenite natively,
  the browser's `WebSocket` in `web.rs`), plus the `spawn` each executor wants.
- Natively, an `http(s)://` URL (`http_url`) uses the server's HTTP posts instead of a websocket
  (`transport/native/http.rs`). Both go through `HTTPS_PROXY`.
- `be_client::authorize` registers an `Authorization` header for a server's origin, which every
  native connection to it sends. A browser cannot add one to a websocket.
- `Connection` is the same code everywhere. It waits on its command channel and its socket rather
  than polling either.

### Peer operations

| Method | Does |
|---|---|
| `open` | Reads the head commit, fetches its chunks and decodes. |
| `save` | Chunks the content, uploads only what `MissingObjects` says the server lacks, publishes against the head it read. |
| `fetch` | Verifies returned bytes hash to the requested hash. The server is not trusted with content: do not drop this check. |
| `fetch_history` | Walks a commit's parents into the local store. Call it before asking history anything (`resume`, `common_ancestor`), or an unfetched ancestor looks like divergence. |
| `stream_header`, `stream_range` | Read a `Streamed` type without the payload. |
| `stash` | Writes a side of a conflicting merge as its own commit. |
| `hold` | `HoldObjects`: keeps objects alive regardless of the block's own retention, until the block is collected. |
| `prune` | Runs retention. |
| `collect_detached` | `CollectDetached`: removes every detached block the caller may edit, with subtree and objects. Nothing calls it yet. |

### Live sessions

`Live<S, C>` is a block being edited in a session.

- The owner sequences and broadcasts. A follower applies its own edit immediately, rebases it
  against operations that arrive in between, and reconciles against the owner's echo.
- It keeps `confirmed` (server-ordered) and `visible` (`confirmed` plus pending).
- `seal` writes a commit, heartbeats `clean_at`, and tells followers with `SessionMessage::Sealed`
  which commit now holds which sequence number.
- `reconcile` runs the resume decision and merges when it has to. It merges unsaved work into
  anything it fast-forwards or merges onto, and when it changes the owner's content outside the
  operation stream, the next `Sealed` tells followers to reload the head.
- Only an owner seals or reconciles in the app's worker: a follower's edits are the owner's to
  save.

### Takeover

A follower's `head` is right only because of `Sealed`, and ownership depends on it. A follower
that takes over:

- carries on from the head the last owner sealed;
- keeps the old sequence numbering, so other followers keep applying what it accepts;
- accepts its own pending operations as its own;
- owes a seal for whatever the old owner accepted after its last one.

With a wrong head, the new owner's first save is refused and the resume decision reads its unsaved
work as something to fast-forward over.

### Conflicts and incompatibility

- A merge that conflicts first writes the owner's unsaved side as its own commit (`Peer::stash`),
  so the merge commit has both sides as parents and neither is lost. It records a `Conflict` the
  worker collects with `take_conflicts`.
- A session message or operation this version cannot decode marks the session incompatible. On a
  follower it also marks it diverged, since its copy may now be wrong; a diverged session refuses
  to edit or seal.

## Version control

Two content types and a peer-side operation set, not a store of its own: history is be-store
objects and be-commit commits.

- **Repository** (`be_block::Repository`): branch heads and an optional upstream repository in
  the same workspace. It holds its history with `Peer::hold`.
- **Snapshot**: a `be_commit::Commit` of kind `Snapshot(message)` whose manifest is a
  `be_vcs::Tree`, one entry per block of the versioned subtree, keyed by local id, with content
  type, parent, metadata, references and content manifest. The manifest is the block's own, so a
  snapshot of unchanged content stores nothing new, and a checkout publishes a block's first
  commit with that manifest without downloading it.
- **Checkout** (`be_block::Checkout`): the parent of a working tree of ordinary blocks, which every
  editor opens and every session edits as usual. Sharing the checkout block is how two people
  work on one tree; two checkouts are two people working apart. It records its repository,
  branch, base commit, root, each block's head at the base (so status compares against the mirror
  instead of downloading) and any conflicts.

### Scopes

A block's content stores the ids of blocks it names, which must mean the same in every checkout,
so they are local ids.

- A block copied into a checkout is created as `masked(local, scope_mask(checkout))` and carries
  the `LOCAL_ID` metadata key. A block created inside a checkout, or adopted into it, keeps its
  own id as its local id.
- The host translates every block id crossing the plugin protocol for an instance whose block is
  in a checkout (`Instances::translate`, over `Message::visit_block_ids`), and the peer resolves
  references it publishes the same way (`Peer::set_resolver`). Neither content nor editors know
  about checkouts.
- Resolution looks in the instance's own checkout first and falls back to the block with that id,
  so a reference out of the tree keeps working.

### Operations

`crates/block-app/src/be/version.rs` runs them in the worker: adopting a block into a new
checkout, committing (a compare-and-swap on the branch), making a checkout, switching, branching,
bringing the branch's changes in, resolving, forking, pulling and pushing.

- Bringing changes in is `be_vcs::merge` over the trees, then each type's `Merge::merge3` for a
  block both sides changed. A conflicted block leaves `base`, `yours` and `theirs` copies under
  the checkout.
- It pushes a `VersionStatus` for every repository and checkout an editor shows, recomputed when
  the mirror changes. It watches the checkout's blocks; the mirror keeps each block's head from
  `HeadChanged`, from sessions and from its own writes.
- A live session's owner catches up when someone publishes past it (`HeadChanged` ->
  `Live::catch_up`), which lets a checkout replace the content of a block someone has open.

## What is not built yet

Version control:

- A commit is made from each block's head after this peer seals what it owns, so an edit another
  peer holds unsealed in a session is left out. A seal barrier would fix it: the server asks the
  owner of every session in the checkout to seal, and the commit waits.
- Conflicts are resolved a block at a time by taking one side. Nothing merges inside a block by
  hand, and the three sides are linked, not shown side by side.
- A repository never lets go of history: deleting a branch frees nothing until the repository
  block is collected. Freeing it wants the server to count holds per object, a commit holding its
  parents and its tree.
- An upstream is another repository in the same workspace. Across workspaces the content keys
  differ, so a pull would have to reseal every object.
- Moving a block into or out of a checkout keeps the ids its content stores, so what they name
  changes with the scope it lands in.

The app's peer:

- Text editor undo lives in the editor, so it is gone when the editor closes, and the app's undo
  command does nothing for text.
- Detached blocks are never collected. When something calls `Peer::collect_detached`, it will
  remove detached blocks that are still referenced and leave the reference dangling: liveness is
  the parent chain by design, so content that references a block has to cope with it being gone.
- The browser peer keeps objects in memory: a reload refetches everything, and a closing tab
  loses what a session held since its last autosave, since `flush()` cannot wait there. An
  `ObjectStore` over IndexedDB is what that wants.
- Reconnecting rejoins from the server's head, so operations unsealed when the socket dropped are
  gone. Recovering them wants the resume path (`clean_at`, `Peer::fetch_history`) be-session
  already has.
- The app deletes its local caches when its build id changes; once the app is offline-first they
  have to stay.

Security and privacy:

- Nothing vouches for who wrote an item. The deterministic game stores the account behind each
  move as a field the sender fills in, so a player can move as someone else. Signing operations
  is not enough, because the session owner sequences and relays everything: stored items have to
  carry their own signatures, checked by whoever reads them. Needed once a chat block makes it
  matter.
- A workspace has one key for its whole life. Removing a member does not rotate it, and sealed
  objects carry no key id to rotate by. Sharing a block across workspaces does not share a key of
  its own.
- The server learns the graph's shape, object sizes and timings, never content. If graph privacy
  matters later, block ids can be blinded per workspace, but the server still needs some graph
  for access inheritance.

Elsewhere:

- Nothing reads content but the editor holding it: no preview built outside an editor and no
  search. Server-side search is impossible under end-to-end encryption, so search has to be
  client-side over the local cache, or a client-built encrypted index.
- Sessions relay through the server. Direct peer connections are a latency optimisation on the
  same protocol and can come later.
