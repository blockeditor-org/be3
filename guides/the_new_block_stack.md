# The block stack

Every block in the app lives in the `be-*` crates: `be-store`, `be-commit`,
`be-session`, `be-graph`, `be-protocol`, `be-model`, `be-block`, `be-client` and
`be-server`. The app is a peer of its own (`crates/block-app/src/be.rs` and
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
- **References cost a workspace per keystroke.** The client replayed pending
  operations to diff the full reference set, and the server rewrote every
  reference row in the workspace on every update. Under a CRDT the delta was
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
- Access is computed per member (`Visibility` in `blocks.rs`, over
  `BlockGraph::access_map`). A workspace administrator edits everything. Anyone
  else gets `Edit` on what they authored and whatever they were granted, and
  both are inherited down through parents; a block they can see references
  gives them `KnowExists` on it, and so does being an ancestor of anything they
  know of, so the path to a shared block is visible without its content.
  `SetAccess` grants, and only someone who may edit a block may grant on it.
- Graph announcements are per member too: when a block changes, every
  connected member of the workspace is sent the `BlockChanged` their own access
  lets them see, or a `BlockRemoved` when it no longer does.
- Accounts live here. `Register` (refused with `--disable-registration`),
  `Login`, `Authenticate` with a token and `Logout`; workspaces are created
  through it, and an administrator can `Invite` an email that the invitee sees
  with `ListInvitations` and answers with `RespondInvitation`.
  `--add-account EMAIL NAME PASSWORD WORKSPACE` provisions an account and its
  workspace from the command line.
- The graph is cached per workspace in memory and dropped on any error so it
  reloads from the database rather than drifting.

### be-block

A block's content is a set of traits a type opts into.

```rust
trait BlockContent {                    // durable form and the edges it declares
    const CONTENT_TYPE: Uuid;
    fn encode(&self) -> Vec<u8>;
    fn decode(bytes: &[u8]) -> Result<Self, ContentError>;
    fn references(&self) -> Vec<Uuid> { Vec::new() }
    fn references_in(&self, workspace: Uuid) -> Vec<Uuid> { self.references() }
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
lets a reader take the header and then a byte range. `TextContent` and every
`Blob` (below) are streamed.

`references_in` is `references` for one workspace, and it is what the peer
calls when it records a commit's references. Text uses it: a block URL names its
workspace, and a URL pasted from another workspace is a link, not a reference.
The URL format itself lives in `be_block::block_url`.

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
keeps `confirmed` (server-ordered) and `visible` (`confirmed` plus pending). `seal` writes a commit, heartbeats
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

A field is one of five things. A `Count` is a counter whose concurrent changes
add up. A `Grid<T>` is a dense block of fixed-size cells addressed by
coordinates, for pixel data: its bounds are part of its value, so a resize moves
the bounds and keeps every cell at its coordinates, `paint` sets cells and
`reshape` sets the bounds. A `List<T>` holds objects of a `Model` type `T`. A `Map<K, V>` holds
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
- **Grids.** A paint is a list of cells, each optionally conditional on what the
  cell held, so a paint's undo repaints only the cells nobody has painted since,
  and a burst of paints into one grid undoes as one stroke. An offline merge
  takes the bounds the way it takes a register and then merges each coordinate
  on its own, so a paint made during a resize lands where it was painted, and a
  crop's undo repaints what the crop dropped.

`Document<R>` implements `BlockContent`, `LiveEdit`, `Merge` and `Undo` in
`be-block` (`model.rs`), so a type built this way is registered with
`kind_with_history` and has undo from the start. Almost every content type is
built this way: the counter, the checklist, the calendar, the browser
tab, the UI settings, the three database types, the presentation, the hotbar,
the deterministic game, the map, the video, the logic game, the logic grid,
compiled logic, the infinite canvas and pixel art. Text implements the
traits by hand, which remains possible for content that does not fit. The browser tab shows a register holding an
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
components merge field by field and its wires, stored as a set of segments,
merge segment by segment and are normalized when read. The canvas keeps its
layering as the order of its entity list, so bringing an entity to the front is
a move. Pixel art keeps its pixels in a `Grid`. The database schema shows the other direction: its fields and enum
options are objects, and their ids are the ids a database's cells and enum values
store, so renaming a field or an option changes nothing that points at it.

A file is the other shape that does not fit a document: a small header and a
payload that can be megabytes. `Blob<K>` is that shape once, for any `BlobKind`
that names a content type and a header: `ImageContent`, `AudioContent`,
`PdfContent`, `GameModuleContent` and `PaintSnapshotContent` are each a `Blob`
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
`block_ui_test::ContentHarness`, which holds the content of the editor's block
and of any block it watches, applies what the editor sends, and takes the
content it seeds or replaces; `ContentStore` is the same store handed to a test
fixture that needs to read or write it between runs.

## Running it

```
./scripts/buck run //crates/be-server:be-server-bin -- --add-account you@example.com "You" hunter2hunter2 Workspace
./scripts/buck run //crates/be-server:be-server-bin -- --addr 127.0.0.1:9090 --data-dir be-server-data
```

`--addr` (or `--address`) defaults to `127.0.0.1:9090` and `--data-dir` to
`be-server-data`. The desktop app does not need a server of its own for a local
account: it embeds be-server (`crates/block-app/src/platform/native.rs`) on an
ephemeral port with a data directory under the app's, and signs in to it like
any other server. An account on another server connects to that server's URL
instead, and the web build always does (`scripts/internal/run-web.sh` starts
`cargo run -p be-server -- --disable-registration` beside it).

Tests start their own server on an ephemeral port; see `Harness` in
`crates/be-client/src/tests.rs` and `crates/be-server/src/tests.rs`. The app's
own tests do the same through the embedded server, register an account and
create a workspace, and then start the app's peer against it the way the app
does: `Harness` in `crates/block-app/src/be/tests.rs`.

## How the app hosts content

The app is the peer, not the plugin. A plugin never sees the content key, the
connection or the server; it is handed content and graph answers by the host,
and hands back operations and graph commands.

### The peer and its registry

`KINDS` in `crates/block-app/src/be.rs` is the only list of content types the
app knows. Each entry is `kind::<C>()`, or `kind_with_history::<C>()` for a type
that implements `Undo`, and it gives the worker the functions it needs without
naming the type: join a session, copy, seed, replace, derive a name with
`BlockContent::name`, and turn a `ChildChange` into operations. A block whose
content type is not in `KINDS` has no content in the app: the host opens
nothing for it and refuses content written to it.

`crates/block-app/src/be/worker.rs` is the peer's loop, with a `Live` session
per open block behind the `Session` trait, so the loop never names a content
type; `be/native.rs` runs it on a thread of its own with a `FileStore` under it,
and `be/web.rs` runs it on the browser's own executor with a `MemoryStore`,
because there is no file system to keep objects in there and every open fetches
what it needs.

### The graph mirror

The app keeps a mirror of the server's graph, as the account sees it, in
`be/graph.rs`: a `Node` per block with its content type, author, parent, access,
references and unsealed `BlockMetadata`. The worker loads it with
`Peer::list_blocks` whenever it connects and keeps it current from the server's
`BlockChanged` and `BlockRemoved` events, reloading it whole when it has fallen
behind the event broadcast. `Query` (`Roots`, `Detached`,
`Children`, `References`, `Backrefs`, `Parents`, `Block`) is how everything in
the app asks it a question, and `be::graph_revision` says when to ask again.

A change made here - `be::create`, `be::set_parent`, `be::set_metadata` - is
applied to the mirror straight away, so the file tree and every editor see it in
the next frame, and counts as pending for that block. While a block has a
pending change the mirror ignores the server's echoes for it, which would
otherwise show a state older than the one on screen; the worker settles the
change when the server has answered, and refreshes the block from the server
once nothing is pending.

### Names

A block's name lives in its metadata (`be_block::BlockMetadata`: `name`,
`named_by_hand` and, for a dynamic artifact, the `ArtifactSource` it was made
from), which the peer seals with the content key before the server stores it
(`Peer::seal_metadata`). `be::set_name` names a block by hand, and clearing it
hands the name back to the content, which renames the block the next time an
editor sees a revision. `be::name_implicitly` is the automatic name:
whenever an instance that may edit a block is sent a new revision of it, the
host derives `BlockContent::name` from the content and writes it, unless the
name was set by hand. The file tree, the block picker and the top bar read the
name out of the mirror, so a block nobody has open keeps the name it was last
given.

### Content on the plugin protocol

Content crosses the plugin protocol in four messages. Each names the block it is
about, because an editor can work on more than its own block.

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
is in `KINDS`, keeps a content link per block per instance beside the editor's
own, takes an `Operate` for any of them only if the account may edit that block,
and closes a block in the peer when no instance holds it any more. An editor
that follows a block chosen by its content rather than a fixed one uses
`editor.related_content::<C>(block)`: given a `Memo<Option<Uuid>>` it projects
whichever block that currently names. A database view follows its database and
the database's schema this way.

Operations, not snapshots, are the normal case. `Live` journals how its visible
content changed (`Journaled::Edited`, `Applied` or `Replaced`), the worker tags
each edit with the origin the host gave the instance that sent it
(`be::operate_from`), and each block's `Content` keeps the last few hundred
operations beside its bytes. `be::update_since` sends an instance the operations
it has not seen, and falls back to a snapshot only when it is further behind
than the log reaches or the content was replaced outright: a join, a reconcile
or a merge. The worker only re-encodes a block when its journal says it changed.

On the plugin side that is `editor.block_content::<C>()`, one `ContentProjection`
per editor. `operate` applies an operation to what the view sees and queues it
for the host. An operation that comes back `mine` only confirms what is already
shown; one from anyone else is applied on top of it, unless an edit of this
instance's is still in flight, in which case the visible state is rebuilt from
the confirmed state and the pending edits. Applying an operation reports what it
touched (`LiveEdit::apply_touching`), and a projection only runs when something
it watches was touched; `guides/reactive.md` says how `project`, `field`, `ids`
and `object` narrow that. `ContentProjection::read` answers `None` until the
host has sent the content once, `loaded()` is a signal that turns true when it
has, and `revision()` counts the changes an editor has seen, for code like the
PDF pane that re-renders on a change rather than projecting.

`block_editor_plugin` re-exports `be_block`, so an editor names its content type
without depending on the crate itself.

### The graph on the plugin protocol

A plugin asks about the graph with `EditorMessage::WatchBlocks { queries }`, a
list of `BlockQuery`s the host answers from its mirror with `Blocks { query,
blocks }` whenever the answer changes (`plugin_host/graph.rs`). A `BlockInfo`
carries the block's type, author, parent (`BlockLocation`: `Root`, `Detached` or
a block), name and whether it was named by hand, references, the account's
access and the artifact source. It changes the graph with
`CreateBlock { block_id, content_type, parent, name, artifact, content }`,
`SetParent` and `SetName`, which the host takes only when the account may edit
the block and the parent it names, and hands to `be::create`, `be::set_parent`
and `be::set_name`. On the plugin side these are `editor.watch_blocks(query)` and
the `Blocks` handle from `editor.blocks()`.

### Creating, seeding and replacing

A new block's first content travels with it: `CreateBlock` carries the encoded
content, and the worker creates the block, then writes the content as its first
commit, before any later command for that block runs. `Creation::create` and
`Blocks::create` make a block this way, Detached unless a parent is named;
the host sets the parent of a block made in a creation dialog once it has the
id. Creating a database makes two blocks this way:
`block_editor_plugin::database::create_database` creates a schema with a Name
field and a database pointing at it, and makes the schema a child of the
database, so the graph is right before any editor opens it.

`editor.seed_content(block, &content)` sends `EditorMessage::SeedContent` for a
block that already exists; the peer writes it as the block's first commit, and
ignores it for a block that already has content or is open, so it cannot
overwrite anything.

Replacing a block's whole content is `editor.replace_content(block, &content)`,
which sends `EditorMessage::ReplaceContent`; the host takes it only for a block
the account may edit. The worker hands it to the open session's
`Live::replace`, or saves it straight to the server when nothing has the block
open. `Live::replace` publishes the new content as a commit based on the head it
knows, retrying on the newer head a rejection names, so the bytes go through the
chunked upload rather than the session. An owner that replaces settles on that
commit and tells its followers to reload; a follower sends the owner
`SessionMessage::Replaced { head }`, and the owner reloads from that head,
reapplies whatever it had sequenced but not sealed, and seals, so a header edit
made while someone was replacing the file is not lost. The image, audio and PDF
editors' "replace" buttons and the pixel art export's regeneration all go this
way. `content_file_creation::<C>` is the creation view that makes a block of a
file type from the file the user picks.

### Children, duplication and undo

Moving a block into a container, deleting a child or replacing it with a copy
asks the container's content to change. `PluginEditor` sends that to
`be::change_child`, and the content type answers with `Root::child_edit`, which
turns a `be_block::ChildChange` into an ordinary edit: a presentation adds or
drops a slide, a database clears or rewrites the cells that link the block, a
hotbar unpins or repoints a component. If the block is not open, `change_child`
holds it and answers `None`, which the callers treat as "not yet" and retry.

Duplicating a block is `be::duplicate`: it adds the copy to the mirror with the
source's type, references and metadata (without its artifact source), and the
worker copies what the source's session shows, or its head when it is not open,
into the copy's id before anything opens the copy.

The app can read a block itself, not only through an editor: the zoom in
`sync_ui_settings` comes from the UI settings block's content. `be::hold` opens a
block for the app and keeps it open when the last editor showing it closes,
because `be::close` leaves a held block alone. Nothing releases a held block
before the stack stops, which is when the workspace changes.

Undo lives in the app's peer, not in an editor, because it is the one place that
sees every edit to a block from this device, whichever editor made it. A content
type registered with `kind_with_history` gets a session that keeps an undo and a
redo stack: every operation an editor sends records a step, a step that arrives
within 750 ms of the last one may be absorbed into it, and `be::undo` and
`be::redo` turn a step into operations the session applies like any other edit.
Plugins reach it with `BlockCommand::Undo` and `BlockCommand::Redo`, and watch
whether either is possible with `EditorMessage::WatchHistory`, which the host
answers with `HistoryStates` whenever they change.

### What to keep true

The peer writes out what its sessions hold before it goes away: `flush()` seals
and waits, `stop()` does the same and then joins the worker, and the close
handler and every workspace change go through one of them, because a session
that is merely dropped loses everything since the last autosave
(`status().unsealed` is what says whether anything is outstanding). The host
refuses an `Operate`, a `ReplaceContent` and a graph command for a block the
account may only read: the plugin holds its own edits behind `editable()`, but
the host does not trust it to, so `Instances::editable` asks the mirror's
access. And nothing in the worker runs on a timer. It waits on its command
channel and on the connection's broadcast, with one deadline for the autosave a
dirty session is owed, so a workspace nobody is editing costs nothing;
`status().wakes` counts the times it woke, and a test asserts that an idle peer
never does.

A connection that drops does not take the peer with it. `Connection::closed` is
a watch the worker waits on beside its commands - the event broadcast outlives
the socket, because the connection holds a sender of its own, so that is not the
signal - and losing it leaves the last content on screen, backs off, connects
again, reloads the graph and rejoins every block that was open. What a session
had not sealed when the socket went is lost: recovering it wants the resume path
(`clean_at`, `Peer::fetch_history`) that be-session already has and the app
does not use yet.

### Reaching the server

The app talks to be-server directly. `crates/block-app/src/accounts.rs` is the
account client: it connects to `{server}/api/be` (be-server accepts the
websocket on any path, so the suffix only matters behind a proxy that routes on
it), registers, logs in and out, lists and creates workspaces, and invites and
answers invitations. What comes back is an account id and a token, and the
app's peer connects with `Credentials::Token` for the workspace it opens.

The content key is not a secret yet. `Config::content_key` derives it as a hash
of a fixed label and the workspace id, so every device of a workspace reads the
same bytes without exchanging anything, and so could anyone who knows the
workspace id, the server included. Content, commits and block metadata are all
sealed with it, so "the server cannot read it" is true of the design and not
yet of the key; the key wrapping below is what replaces it.

### Presence

Presence is what a peer shows the others while it is there - a cursor, a
selection - and it is never saved. `SessionMessage::Presence { kind, value }`
carries one kind of it to everyone in the session, sealed like every other
session message. `Live::set_presence` shows a value (or takes it away with
`None`) and only sends when it changed, `Live::presence` is what the other peers
show, keyed by their `ClientId` and the kind, and a peer never sees its own.
When `SessionChanged` names a participant it has not seen, a peer shows that
participant what it is showing, so a late joiner sees everyone at once; when a
participant leaves, what it showed is dropped.

The worker publishes each block's peers into `Shared::presence` when a session
says they changed, and the host passes them to the instances holding that block
as `EditorMessage::PeerPresence`. A plugin shows presence with
`EditorMessage::ShowPresence`, which the host takes from any instance that holds
the block and may view it, and takes away again when the instance closes. On the
plugin side that is `editor.show(Some(&value))` and `editor.peers::<P>()`, a
signal of every peer's value of one `PresenceKind` (`be_block::presence`). The
framework shows a `UserActive` value of its own, with a colour picked by
`pick_free_color` against the colours already showing, whenever the host says the
block is visible. The text editor's cursors and the canvas's pointers and
selections work the same way, and each carries a colour of its own picked the
same way rather than joining it to the peer's `UserActive` entry.

### Editors with their own model

The text editor cannot hand its state to a projection: `text_editor_core` wants
a `Document` with an anchor per byte, so a cursor stays on its character while
other people type. `text_block::document::BlockDocument` keeps the bytes and
their anchors itself, turns each local edit into `TextOp::Delete` and
`TextOp::Insert` that the editor pushes into its `ContentProjection` every
frame, and adopts a change from elsewhere by diffing the projection's text
against its own: unchanged bytes keep their anchors, and only inserted bytes get
new ones. Its undo is its own too, because it has to give the text core back its
cursors. Each step remembers the anchors either side of the text it replaced
and both versions of that text, so undo finds the text wherever it has moved to
and skips it when someone else has changed it since.

Dynamic artifacts reach content from the artifact session through the
host: `EditorHost::content_of` gives a regeneration a projection of its source,
and `EditorHost::replace_content` writes the result. Compiled logic and the
pixel art export regenerate this way.

## What is not built yet

- Version control was removed with the old block system and has to be remade on
  this stack. be-commit already keeps the history it would read: every block is
  a commit chain, and `Peer::fetch_history` walks it.
- Undo in the text editor lives in the editor, so it is gone when the editor
  closes, and the app's undo command does nothing for text.
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
- Detached blocks are never collected by the app. be-server's
  `CollectDetached` (`Peer::collect_detached`) removes every detached block the
  caller may edit, with its subtree and its objects, but nothing calls it. When
  something does, it will remove a detached block that is still referenced and
  leave the reference dangling: liveness is the parent chain, by design, so the
  content that references it has to cope with a block that is gone.
- The browser peer keeps its objects in memory, so a reload refetches everything
  and a tab that closes leaves whatever a session held since its last autosave
  behind: `flush()` cannot wait there. An `ObjectStore` over IndexedDB is what
  that wants.
- Reconnecting rejoins from the server's head, so operations a session had not
  sealed when the socket dropped are gone.
- The content key is derived from the workspace id (see Reaching the server), so
  it protects nothing from the server yet. Keys are passed in whole
  (`ContentKey`); there is no per-recipient key wrapping, so sharing a block
  across accounts or workspaces does not share a key of its own.
- Sessions relay through the server. Direct peer connections are a latency
  optimisation on the same protocol and can come later.
- The server learns the shape of the graph, object sizes and timings. It never
  learns content. If graph privacy matters later, block ids can be blinded per
  workspace, but the server still needs some graph to do access inheritance.
- Retention is driven by the client calling `Peer::prune`. Nothing schedules it.
- An empty session is remembered while it carries a `clean_at`, which is bounded
  by the number of blocks that have been edited live. If that grows, evict by
  age rather than dropping the marker.
