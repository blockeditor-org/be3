# The new block stack

The `be-*` crates are a replacement for `block`, `block-server` and
`block-client`. They live beside the old stack rather than inside it: the old
stack still runs the application, and five editors - the counter, the
checklist, the browser tab, the UI settings and the calendar - keep their
content in the new one. Work on them by migrating one thing at a time, not by
rewriting the app around them.

If you are changing an existing editor or block type today, you want
`guides/adding_a_block.md` and the `block` crate. This guide is for work on the
replacement.

## Why there is a replacement

The old `Block` trait fuses five separable things into one type: the durable
representation, the live-edit operation, undo history, the reference graph, and
child manipulation. `const CRDT: bool` then makes a single choice that governs
live merge, offline merge and the wire protocol together. Most of the problems
below follow from that coupling.

- **The operation log never compacts.** `snapshot_seq` is written as `0` when a
  block is created and nothing ever advances it, so reading a block replays
  every operation it has ever received. It cannot be fixed in place either:
  operations are ciphertext to the server, and compaction needs understanding.
  An append-only operation log and end-to-end encryption are incompatible.
- **References cost a workspace per keystroke.** The client replays pending
  operations to diff the full reference set, and the server deletes and rewrites
  every `block_references` and `block_properties` row in the workspace on every
  update. Under a CRDT the delta is also wrong, because it is computed against
  an optimistic local state rather than the converged one.
- **There is no local persistence.** The old client keeps unsent work in memory.
  Closing the process loses it, so there is no offline story to preserve.
- **Bulk data is in band.** The old image block holds a `Vec<u8>` that is
  base64'd into JSON, wrapped in a snapshot, and shipped whole on every replace.
- **The layering is inverted.** `version_control_object` and
  `version_control_data` already implement a content-addressed blob store with
  commits and branches, as block types on top of the block system. The thing
  that should be the substrate is a guest.

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
be-client    a peer: local store, blocks, Live    client
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
- `Vault::seal` derives its nonce from the key and the plaintext. That is
  deliberate: it makes identical content deduplicate under one key, and share
  nothing across keys. Do not replace it with a random nonce without replacing
  the deduplication story too.
- `Manifest` is `{content_type, length, chunks}`. `Vault::read_range` and
  `chunks_in_range` are the streaming primitives.
- `ObjectStore` is `MemoryStore` or `FileStore`. It is synchronous on purpose;
  network fetching is explicit (see `Peer::fetch`), so nothing blocks inside it.

### be-commit

- `Commit` is `{parents, manifest, author, time, kind, references}`, stored as an
  encrypted object whose hash is its `CommitId`. Several parents means a merge.
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
  stomping a newer owner.
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
`BlockParent::Root`; a block with no parent is pending deletion **together with
its subtree, even when something else still references it**. References are for
navigation and access, never for liveness.

That is what makes reclamation reference counting rather than tracing: a block
holds its commits, a commit holds its chunk hashes, and `ObjectRefs` deletes an
object when the last holder releases it. There is no cycle to trace and no mark
and sweep. Do not reintroduce one.

### be-server

SQLite for metadata, a `FileStore` for objects, and a websocket carrying
postcard frames.

- A publish is a compare-and-swap on the head. A publish against a stale head
  comes back `Rejected` with the head to merge against, rather than being
  silently ordered after it.
- A publish writes one head update, the handful of edge rows the commit
  declares, and refcounts for chunks it actually introduced. It does not rewrite
  the workspace.
- Workspace membership is the trust boundary: every member has `Edit` on every
  block in the workspace. Per-block grants are recorded and reported but do not
  restrict members yet; they are the hook for sharing with non-members.
- The graph is cached per workspace in memory and dropped on any error so it
  reloads from the database rather than drifting.

### be-block

The old `Block` trait split into parts a type opts into.

```rust
trait BlockContent {                    // durable form and the edges it declares
    const CONTENT_TYPE: Uuid;
    fn encode(&self) -> Vec<u8>;
    fn decode(bytes: &[u8]) -> Result<Self, ContentError>;
    fn references(&self) -> Vec<Uuid> { Vec::new() }
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

History and child manipulation are gone from the trait. Undo is client-local and
against a sequencer it emits an inverse operation rather than rewinding state;
children are graph operations. `Undo::step` is taken from the state before an
operation, and `revert` and `reapply` turn a step into operations against the
state as it is *now*, so an undo leaves alone whatever someone else changed
since: the calendar undoes a rename without moving an event another peer
rescheduled.

`Streamed` types encode as `[u32 header length][header][payload]`, which is what
lets a reader take the header and then a byte range. `ImageContent` and
`TextContent` are the two ported types.

### be-client

`Peer` is a local object store plus a connection. `transport/` is what makes
that connection portable: a `Writer` and a `Reader` per platform - tokio and
tokio-tungstenite natively, the browser's own `WebSocket` in `web.rs` - plus the
`spawn` each one's executor wants. `Connection` above it is the same code
everywhere, and it waits on its command channel and its socket rather than
polling either.

- `open` reads the head commit, fetches its chunks and decodes.
- `save` chunks the content, uploads only what `MissingObjects` says the server
  lacks, and publishes against the head it read.
- `fetch` verifies that bytes the server returns hash to the hash that was asked
  for. The server is not trusted with content, so do not drop this check.
- `fetch_history` walks a commit's parents into the local store. Anything that
  asks history a question (`resume`, `common_ancestor`) needs this first, or an
  unfetched ancestor looks like a divergence.
- `stream_header` and `stream_range` read a `Streamed` type without the payload.

`Live<S, C>` is a block being edited in a session. The owner sequences and
broadcasts; a follower applies its own edit immediately, rebases it against
operations that arrive in between, and reconciles against the owner's echo. It
keeps `confirmed` (server-ordered) and `visible` (`confirmed` plus pending),
which is the same shape the old client used. `seal` writes a commit, heartbeats
`clean_at`, and tells the followers with `SessionMessage::Sealed` which commit
now holds which sequence number. `reconcile` runs the resume decision and merges
when it has to.

A follower's `head` is only right because of `Sealed`, and ownership depends on
it being right. A follower that takes over carries on from the head the last
owner sealed: it keeps the old sequence numbering so the other followers keep
applying what it accepts, it accepts the operations it had pending as its own,
and it owes a seal for whatever the old owner accepted after its last one.
Get the head wrong and the new owner's first save is refused, and the resume
decision reads its unsaved work as something to fast-forward over. `reconcile`
now merges unsaved work into anything it fast-forwards or merges onto, and when
it changes the owner's content outside the operation stream, the next `Sealed`
tells the followers to reload the head. Only an owner ever seals or reconciles
in the app's worker: a follower's edits are the owner's to save.

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

A field is one of four things. A `Count` is a counter whose concurrent changes
add up. A `List<T>` holds objects of a `Model` type `T`. A `Map<K, V>` holds
values by key, each key its own register: two people setting different keys
both keep theirs, which is how a database row holds a cell per schema field.
Anything else that is `Serialize + DeserializeOwned + Clone + PartialEq +
Default` is a register: it is set as a whole, and setting it on both sides of an
offline merge is a conflict. `Root` names the content type and, optionally, the
block's name and the blocks it references.

`be-model` stores a document as a table of objects with ids, not as a tree of
values, and every algorithm is written once against that table:

- **Identity.** Every object in a `List` has an `ObjectId` (the `id` of the
  `Item` the list reads back as), and edits name the object, not a position.
  An edit to an object that has since moved into another list still lands on
  it, and a `move_into` carries an object and everything under it from one list
  to another.
- **Edits.** The derive gives each field a typed constant, `CalendarEvent::TITLE`,
  that builds the changes an `Edit` is made of: `set` for a register, `add` for a
  `Count`, `put` for a key of a `Map`, `insert` and `move_into` for a `List`, and
  `Change::remove` for any object. A content type usually wraps these in helpers the editor calls, like
  `Calendar::update`, which only writes the fields that changed.
- **Live editing.** Edits address objects by id and anchor inserts to a sibling,
  so they mean the same thing whatever the sequencer put before them: there is
  nothing to rebase.
- **Offline merge.** `Document::merge` matches objects by id across the three
  versions and merges each field on its own: registers take whichever side
  changed, counts add both sides, lists merge their order the way `merge_slices`
  merges lines, and an object that moved on one side and was edited on the other
  keeps both. Deleting an object that the other side edited, or that the other
  side put something into, keeps it and counts a conflict rather than losing the
  edit.
- **Undo.** `Document::step` records, for each change, the change that undoes it
  and the one that redoes it, both taken against the state before the edit. A
  register's undo is conditional (`Change::SetIf`, and `Change::PutIf` for a map
  key): it only puts the old value back if nobody has changed it since, which is how undo leaves other
  people's edits alone. A removed object is put back with everything under it,
  after the sibling it followed. Consecutive sets of the same fields absorb into
  one step.

`Document<R>` implements `BlockContent`, `LiveEdit`, `Merge` and `Undo` in
`be-block` (`model.rs`), so a type built this way is registered with
`migrated_with_history` and has undo from the start. Every migrated editor's
content is built this way: the counter, the checklist, the calendar, the browser
tab, the UI settings, the three database types, the presentation and the
hotbar. Text and images still implement the traits by hand,
which remains possible for content that does not fit, such as a byte payload or
a type that is better as a CRDT. The browser tab shows a register holding an
`Option<ObjectId>`: its current page is an object in its history, not an index,
so a push and a navigation made at the same time still agree on which page is
current. The database schema shows the other direction: its fields and enum
options are objects, and their ids are the ids a database's cells and enum values
store, so renaming a field or an option changes nothing that points at it.

Test a type's helpers in `crates/be-block/src/tests/`; the model itself is
tested in `crates/be-model/src/tests/`, and the round trip through a real server
in `crates/be-client/src/tests/`.

## Running it

```
cargo run -p be-server -- --add-account you@example.com "You" hunter2hunter2 Workspace
cargo run -p be-server -- --address 127.0.0.1:8787 --data-dir be-server-data
```

Tests start their own server on an ephemeral port; see `Harness` in
`crates/be-client/src/tests.rs` and `crates/be-server/src/tests.rs`. The app's
own tests start a `block-server` instead and reach the new stack through it, the
way the app does: `crates/block-app/src/be/tests.rs`.

## The counter, migrated

The counter editor is the first thing in `block-app` that keeps its content
here. It is the shape every later migration should take, so it is worth reading
before starting another one.

Identity stays where it was. `block_client::blocks::counter::Counter` is still
the block type: it is what the new-block menu offers, what the file tree draws,
where the block sits in the workspace, and who may edit it. It stores nothing
any more. The count is `be_block::CounterContent`, held in the new stack under
the same block id, and `crates/block-app/src/be.rs` is the only place that says
which block types that is true of: `MIGRATED` pairs each old block type with its
content type, and `content_type_for` reads it.

The app is the peer, not the plugin. `crates/block-app/src/be/worker.rs` is the
peer's loop, with a `Live` session per open block behind the `Session` trait, so
the loop never names a content type; `be/native.rs` runs it on a
thread of its own with a `FileStore` under it, and `be/web.rs` runs it on the
browser's own executor with a `MemoryStore`, because there is no file system to
keep objects in there and every open fetches what it needs. The plugin never
sees the content key or the connection: it is handed content and hands back
operation bytes, through four messages on the plugin protocol. Each names the
block it is about, because an editor can work on more than its own block.

- `EditorMessage::Content { block_id, content_type, bytes, applied }` - host to plugin, a
  snapshot. `applied` counts the operations *this instance* sent that the
  snapshot already contains, which is what lets a plugin tell its own
  unacknowledged edits from everyone else's.
- `EditorMessage::ContentOperations { block_id, operations }` - host to plugin, every
  operation since the last thing the instance was sent, in order, each marked
  `mine` when this instance is the one that sent it.
- `EditorMessage::Operate { block_id, operation }` - plugin to host.
- `EditorMessage::WatchContent { blocks }` - plugin to host, the other blocks
  (and their content types) the editor wants to follow.

`editor.content_of::<C>(block)` is how an editor reads and edits another block:
it registers the block with `WatchContent` and hands back the same
`ContentProjection` an editor gets for its own block, keyed by that block. The
host opens a watched block only if the account may view it and its content type
is migrated, keeps a content link per block per instance beside the editor's own,
takes an `Operate` for any of them only if the account may edit that block, and
closes a watched block in the new stack when no instance holds it any more.

Operations, not snapshots, are the normal case. `Live` journals how its visible
content changed (`Journaled::Edited`, `Applied` or `Replaced`), the worker tags
each edit with the origin the host gave the instance that sent it
(`be::operate_from`), and each block's `Content` keeps the last few hundred
operations beside its bytes. `be::update_since` sends an instance the operations
it has not seen, and falls back to a snapshot only when it is further behind
than the log reaches or the content was replaced outright: a join, a reconcile
or a merge. The worker only re-encodes a block when its journal says it changed.

On the plugin side that is `editor.block_content::<C>()`, the `ContentProjection`
beside `BlockProjection`, one per editor. `operate` applies an operation to what
the view sees and queues it for the host. An operation that comes back `mine`
only confirms what is already shown; one from anyone else is applied on top of
it, unless an edit of this instance's is still in flight, in which case the
visible state is rebuilt from the confirmed state and the pending edits.

Applying an operation reports what it touched (`LiveEdit::apply_touching`), and
a projection only runs when something it watches was touched. `project` watches
everything; `project_on(key, ...)` watches one key. A hand-written content type
reports `Touched::Everything`; a `be-model` document reports the field it
changed and every object that contains it, so `ContentProjection<Document<R>>`
can offer `field(object, FIELD)`, `ids(owner, LIST)` and `object::<T>(id)`,
which run only for the field, the list or the object (and what is inside it)
they name. The checklist's rows each watch their own item and its list watches
only its order, so ticking one item runs one row. A watcher is dropped with the
reactive scope that made it.

`block_editor_plugin` re-exports `be_block`, so an editor names its content type
without depending on the crate itself.

Migrating another self-contained editor is now four steps: a content type in
`be-block` (see Adding a content type), an entry in `MIGRATED`, the editor
reading `editor.block_content::<C>()` instead of `editor.block::<B>()`, and the
old block type emptied the way `Counter` and `Checklist` are. The browser tab is
the example for an editor that reads its content outside a projection:
`ContentProjection::read` answers `None` until the host has sent the content
once.

A migrated block is still named in the old stack, because that is where the file
tree, the block picker and search look. `BlockContent::name` is the name, and
the host carries it across: whenever an editable instance is sent a new revision,
`Instance::name_from_content` writes it as the block's automatic name through
`BlockHandle::set_implicit_name`, which never touches a name someone set by hand.
An empty automatic name is how a name is taken away, because the old server has
no way to delete a property. This is a bridge, not the design: it shows the old
server every name, exactly as the old stack already does, and it goes away with
the old stack, when names move into an index the server cannot read.

The same bridge carries references, which the file tree, backlinks and
`watch_references` read from the old graph. The old client derives a block's
references from its value, so a migrated type that references other blocks
keeps them in its otherwise empty old value: `Database`, `DatabaseSchema` and
`DatabaseView` each hold a `references` list and nothing else, and
`Block::bridged_references` names the operation that sets it. Whenever the host
bridges a name it also hands `BlockContent::references` to
`BlockHandleAccess::set_references`, for the editor's own block and for every
block it watches, which only writes when the set changed.

The old stack's child hooks reach a migrated block's content too. Moving a
block into a container, deleting a child or replacing it with a copy calls
`add_child`, `delete_child` or `replace_child` on the container's editor, and
for a migrated type `PluginEditor` sends that to `be::change_child` instead of
the emptied old value. The content type answers with `Root::child_edit`, which
turns a `be_block::ChildChange` into an ordinary edit: a presentation adds or
drops a slide, a database clears or rewrites the cells that link the block, a
hotbar unpins or repoints a component. If the block is not open, `change_child`
holds it and answers `None`, which the callers already treat as "not yet" and
retry.

A new block's first content comes from the editor that made it. The old stack
creates the block, and `editor.seed_content(block, &content)` (or the same on
`Creation`) sends `EditorMessage::SeedContent`; the peer writes it as the
block's first commit, and ignores it for a block that already has content or is
open, so it cannot overwrite anything. Creating a database makes three blocks
this way: `block_editor_plugin::database::create_database` seeds a schema with a
Name field and a database pointing at it, and gives the old database block the
schema as its reference straight away, so the graph is right before any editor
opens it.

An editor that follows a block chosen by its content, rather than a fixed one,
uses `editor.related_content::<C>(block)`: given a `Memo<Option<Uuid>>` it
projects whichever block that currently names, the way `related` does for an old
block. A database view follows its database and the database's schema this way,
resolving each `BlockRef` with `editor.resolve` as before. `BlockRef` itself now
lives in `be-block`, and `block_client::block_ref` re-exports it.

Duplicating a block is the old stack's `BlockHandleAccess::duplicate`, which
copies an empty block for a migrated type, so the app follows it with
`be::duplicate`: the worker copies what the source's session shows, or its head
when it is not open, into the copy's id before anything opens the copy.

The app can read a migrated block itself, not only through an editor: the zoom
in `sync_ui_settings` comes from the UI settings block's content. `be::hold`
opens a block for the app and keeps it open when the last editor showing it
closes, because `be::close` leaves a held block alone. Nothing releases a held
block before the stack stops, which is when the workspace changes.

Undo lives in the app's peer, not in an editor, because it is the one place that
sees every edit to a block from this device, whichever editor made it. A content
type that implements `Undo` is registered with `migrated_with_history`, and its
session keeps an undo and a redo stack: every operation an editor sends records
a step, a step that arrives within 750 ms of the last one may be absorbed into
it, and `be::undo` and `be::redo` turn a step into operations the session
applies like any other edit. Plugins reach it without knowing about the new
stack: a plugin asks with `BlockCommand::Undo` and `BlockCommand::Redo`, and
watches whether either is possible with `EditorMessage::WatchHistory`, which the
host answers with `HistoryStates` whenever they change. The workspace falls back
to that whenever the old block's handle has no history of its own, which is
true of every migrated block.

Four things are worth copying. A migrated editor's block type keeps its old
entry in `block_types!` with no state in it, rather than disappearing: the graph
still needs it. The peer writes out what its sessions hold before it goes away:
`flush()` seals and waits, `stop()` does the same and then joins the worker, and
the close handler and every workspace change go through one of them, because a
session that is merely dropped loses everything since the last autosave
(`status().unsealed` is what says whether anything is outstanding). The host
refuses an `Operate` for a block the account may only read: the plugin holds its
own edits behind `editable()`, but nothing between the plugin and the peer knows
about access, so `Instances::editable` asks the old client the same question
`Open` asked. And nothing in the worker runs on a timer. It waits on its command
channel and on the connection's broadcast, with one deadline for the autosave a
dirty session is owed, so a workspace nobody is editing costs nothing;
`status().wakes` counts the times it woke, and a test asserts that an idle peer
never does.

A connection that drops does not take the peer with it. `Connection::closed` is
a watch the worker waits on beside its commands - the event broadcast outlives
the socket, because the connection holds a sender of its own, so that is not the
signal - and losing it leaves the last content on screen, backs off, connects
again and rejoins every block that was open. What a session had not sealed when
the socket went is lost: recovering it wants the resume path (`clean_at`,
`Peer::fetch_history`) that be-session already has and the app does not use yet.

### Reaching the server

A browser has one origin: the one it was served from. So the new stack's server
rides on the old one's port rather than on a second one - `block-server` opens a
`be_server::Hosted` beside its own store and hands it every websocket upgrade
for `/api/be`, which is the same handshake the block socket takes, with the same
`token` and `workspace` query parameters.

That is also what removed the accounts problem. The upgrade has already been
authenticated, so `block-server` carries the account and the workspace across
under the same ids (`Hosted::adopt`, an upsert of the account, the workspace and
the membership), and the peer asks for them with `ClientMessage::Adopt` instead
of registering. A be account id is a block account id, and a be workspace id is
a block workspace id, so there is nothing for the app to store and nothing to
keep in step.

The content key follows from that: it is derived from the workspace id, so two
devices of one workspace read each other's bytes. That is a placeholder, not a
design - it is no more secret from the server than `crypto::STATIC_KEY` is in
the old client - and it is what the key wrapping below replaces.

## What is not built yet

- A migrated block's references reach the old graph only while an editor holds
  the block, like its name.
- A migrated block's content is not in the old workspace index, so nothing but
  the editor can read it: no preview and no search. Only its name is carried
  across, and only while an editor has it open.
- The browser peer keeps its objects in memory, so a reload refetches everything
  and a tab that closes leaves whatever a session held since its last autosave
  behind: `flush()` cannot wait there. An `ObjectStore` over IndexedDB is what
  that wants.
- A plugin still reaches the new stack through the host rather than through a
  peer of its own. `block-client` runs inside a plugin over a tunnel the host
  carries; be-client has the transport split to do the same, but no tunnel.
- Reconnecting rejoins from the server's head, so operations a session had not
  sealed when the socket dropped are gone.
- Keys are passed in whole (`ContentKey`). There is no per-recipient key
  wrapping, so sharing a block across accounts does not yet share its key, and
  `crypto::STATIC_KEY` in the old client has no counterpart here on purpose.
- Sessions relay through the server. Direct peer connections are a latency
  optimisation on the same protocol and can come later.
- Server-side search is impossible under end-to-end encryption. It has to become
  client-side over the local cache, or a client-built encrypted index.
- The server learns the shape of the graph, object sizes and timings. It never
  learns content. If graph privacy matters later, block ids can be blinded per
  workspace, but the server still needs some graph to do access inheritance.
- Retention is driven by the client calling `Peer::prune`. Nothing schedules it.
- An empty session is remembered while it carries a `clean_at`, which is bounded
  by the number of blocks that have been edited live. If that grows, evict by
  age rather than dropping the marker.
