# Blocks in the app

How block-app hosts block content: its peer and registry, the graph mirror, metadata, how content
and graph reach plugins, creating and replacing blocks, children, undo, settings and sessions,
view state, keys and presence. Read it before changing `crates/block-app/src/be.rs`, `be/`, or
the content and graph parts of the plugin protocol. The crates underneath are in
`guides/blocks/stack.md`; what an editor calls is in `guides/plugins/editor_api.md`.

The app is the peer, not the plugin. A plugin never sees the content key, the connection or the
server: the host hands it content and graph answers, and it hands back operations and graph
commands.

## Rules to keep

- **Write out before going away.** `flush()` seals and waits; `stop()` does the same and then
  joins the worker. The close handler and every workspace change go through one of them, because
  a session that is merely dropped loses everything since the last autosave.
  `status().unsealed` says whether anything is outstanding.
- **The host enforces access.** It refuses an `Operate`, a `ReplaceContent` and a graph command
  for a block the account may only read. The plugin holds its own edits behind `editable()`, but
  the host does not trust it: `Instances::editable` asks the mirror's access.
- **Nothing in the worker runs on a timer.** It waits on its command channel and the connection's
  broadcast, with one deadline for the autosave a dirty session is owed, so an idle workspace
  costs nothing. `status().wakes` counts wakes, and a test asserts an idle peer never wakes.
- **A dropped connection does not take the peer with it.** The worker waits on
  `Connection::closed` beside its commands (not on the event broadcast, which outlives the socket
  because the connection holds a sender of its own). On loss it keeps the last content on screen,
  backs off, reconnects, reloads the graph and rejoins every open block. What a session had not
  sealed is lost (`guides/blocks/stack.md`, "What is not built yet").

## The peer and its registry

`KINDS` in `crates/block-app/src/be.rs` is the only list of content types the app knows. Each
entry is `kind::<C>()`, or `kind_with_history::<C>()` for a type that implements `Undo`. It gives
the worker, without naming the type, the functions to join a session, copy, seed, replace, derive
a name with `BlockContent::name`, and turn a `ChildChange` into operations. A block whose type is
not in `KINDS` has no content in the app: the host opens nothing for it and refuses content
written to it.

Paths are under `crates/block-app/src/`.

| File | Role |
|---|---|
| `be/worker.rs` | The peer's loop: a `Live` session per open block behind the `Session` trait, so the loop never names a content type. |
| `be/native.rs` | Runs the worker on its own thread over a `FileStore`. |
| `be/web.rs` | Runs it on the browser's executor over a `MemoryStore`: no file system, so every open fetches what it needs. |
| `be/graph.rs` | The graph mirror. |
| `be/version.rs` | Version control operations (`guides/blocks/stack.md`, "Version control"). |
| `accounts.rs` | The account client. |
| `keys.rs`, `ui/keys.rs` | The unlock screen. |

### Reaching the server

The app talks to be-server directly. `crates/block-app/src/accounts.rs` connects to
`{server}/api/be` (be-server accepts the websocket on any path, so the suffix matters only behind
a proxy that routes on it). It registers, logs in and out, lists and creates workspaces, and
invites and answers invitations. It gets back an account id and a token, and the peer connects
with `Credentials::Token` for the workspace it opens.

## Blocks saved by another build

Until a type is frozen its format changes freely, so a block may hold what an older build saved.

- Every save stamps the derived `SCHEMA` key with `schema_fingerprint::<C>()`, a hash of
  `BlockContent::schema()`: be-model's `Kind::describe` for a document, `blob N` for a blob (whose
  kind bumps `BlobKind::SCHEMA` when old headers would be read wrongly).
- `be::staleness` calls an unfrozen block stale when the worker could not decode it
  (`Shared::unreadable`) or its stamp is another schema's. A block with no stamp is not stale.
- A stale block's frame shows `StaleNotice` (`surfaces::StaleBlock`) instead of its editor:
  - Reset publishes the type's default content (`be::reset`).
  - Delete deletes it as the shell would.
  - Try anyway shows the editor for the rest of the session (`be::try_anyway`).
- A block the app holds itself (`be::hold`: settings, the shell's session) is reset by the worker
  without asking, with a notice, since the app needs it to work.
- The app deletes its local caches (`be-objects/`, `plugin-cache/`) whenever its build id
  changes.

## The graph mirror

`be/graph.rs` mirrors the server's graph as the account sees it: a `Node` per block with its
content type, author, parent, access, references and unsealed `BlockMetadata`.

- The worker loads it with `Peer::list_blocks` on every connect and keeps it current from
  `BlockChanged` and `BlockRemoved`, reloading it whole when it has fallen behind the broadcast.
- `Query` (`Roots`, `Detached`, `Children`, `References`, `Backrefs`, `Parents`, `Block`) is how
  everything in the app asks it a question; `be::graph_revision` says when to ask again.
- A change made here (`be::create`, `be::set_parent`, `be::define`) is applied to the mirror at
  once, so the file tree and every editor see it next frame, and counts as pending for that block.
- While a block has a pending change, the mirror ignores the server's echoes for it, which would
  show an older state. The worker settles the change when the server answers, and refreshes the
  block from the server once nothing is pending.

## Metadata

A block's metadata (`be_block::BlockMetadata`) is two maps from a key's UUID to a CBOR value:

| Map | Holds | Examples |
|---|---|---|
| `defined` | What someone chose | `NAME` (typed by hand), `ArtifactSource`, a checkout copy's `LOCAL_ID` |
| `derived` | What the content says about itself | `INTRINSIC_NAME`, `THUMBHASH`, `SCHEMA` |

- A key is a typed constant, `MetaKey::defined(id, name)` or `MetaKey::derived(id, name)`, read
  and written through it (`get`, `insert`, `remove`, or `definition` for a change to send). A new
  key, even one a third-party type declares, needs no format change.
- `BlockMetadata::name` is `NAME` when someone named the block, else `INTRINSIC_NAME`.
- Plugins get both maps in `BlockInfo`, read them through `name`, `thumbhash` and `artifact`, and
  set a defined key with `Blocks::define`. The host never lets a plugin set `LOCAL_ID`.
- A beui `unstyled::Picture` given an image's thumbhash as a `beui::Thumbhash` lays out at the
  image's size and paints the blurred placeholder until the image arrives.

### How metadata travels

- Every commit carries the encoded metadata of the content it saves, and `Publish` carries it
  sealed with the content key (`Peer::seal_metadata`), so the server stores it in the same step
  that moves the head and cannot keep metadata for a head it refused.
- A save takes `defined` from the head it builds on and recomputes `derived` from the content
  (`be_block::derived_metadata`: `BlockContent::name` and `BlockContent::describe`). A blob type
  describes itself from its header alone, so an image's thumbhash is copied from `ImageHeader`,
  never recomputed.
- A merge after a refused save merges `defined` key by key against the base commit
  (`BlockMetadata::merge_defined`, the published head winning a clash) and derives the rest from
  the merged content. Version control does the same; a clash on `NAME` there is
  `ConflictKind::Name`.
- Defined keys a build does not know ride along unchanged. Derived keys it does not know are
  dropped when it saves new content (they describe the old content) and filled in again by the
  next build that knows them.

### Changing defined metadata

- `Peer::define`: on a block with content, publishes a commit of the current content with the new
  metadata, which a live session catches up with like any outside publish. A block with no content
  yet takes it through `SetMetadata`, which the server refuses once the block has a head.
- In the app, `be::define` (and `be::set_name`, for `NAME`) applies the change to the mirror at
  once and sends it.
- `be::describe_implicitly` keeps an open block's derived metadata current in the mirror as its
  content changes (`Graph::derive`), until the save that publishes it.

## Content on the plugin protocol

Four messages carry content. Each names its block, because an editor can work on more than its
own.

| Message | Direction | Carries |
|---|---|---|
| `EditorMessage::Content { block_id, content_type, bytes, applied }` | host to plugin | A snapshot. `applied` counts operations this instance sent that the snapshot already contains, so a plugin can tell its unacknowledged edits from everyone else's. |
| `EditorMessage::ContentOperations { block_id, operations }` | host to plugin | Every operation since the last thing the instance was sent, in order, each marked `mine` when this instance sent it. |
| `EditorMessage::Operate { block_id, operation }` | plugin to host | An edit. |
| `EditorMessage::WatchContent { blocks }` | plugin to host | Other blocks (and their content types) the editor wants to follow. |

### Operations, not snapshots

- `Live` journals how its visible content changed (`Journaled::Edited`, `Applied` or `Replaced`).
- The worker tags each edit with the origin the host gave the sending instance
  (`be::operate_from`), and each block's `Content` keeps the last few hundred operations beside
  its bytes.
- `be::update_since` sends an instance the operations it has not seen, falling back to a snapshot
  only when it is further behind than the log reaches or the content was replaced outright (a
  join, a reconcile or a merge).
- The worker re-encodes a block only when its journal says it changed.

### The plugin side

- `editor.block_content::<C>()` is one `ContentProjection` per editor. `operate` applies an
  operation to what the view sees and queues it for the host.
- An operation that comes back `mine` only confirms what is shown. One from anyone else is applied
  on top, unless an edit of this instance is still in flight, in which case the visible state is
  rebuilt from the confirmed state and the pending edits.
- Applying an operation reports what it touched (`LiveEdit::apply_touching`), and a projection
  runs only when something it watches was touched; `guides/beui/reactive.md` says how `project`,
  `field`, `ids` and `object` narrow that.
- `ContentProjection::read` answers `None` until the host has sent the content once, and
  `operate` drops an edit made before then. `loaded()` is a signal that turns true when it has
  (`guides/beui/reactive.md` says what an editor shows until then). `revision()` counts changes
  seen, for code like the PDF pane that re-renders on a change rather than projecting.
- `block_editor_beui` re-exports `be_block`, so an editor names its content type without
  depending on the crate.

### Watching other blocks

- `editor.content_of::<C>(block)` registers the block with `WatchContent` and returns the same
  `ContentProjection` an editor gets for its own block, keyed by that block.
- `editor.related_content::<C>(block)` follows a block chosen by content: given a
  `Memo<Option<Uuid>>`, it projects whichever block that currently names. A database view follows
  its database and the database's schema this way.
- The host opens a watched block only if the account may view it and its type is in `KINDS`. It
  keeps a content link per block per instance beside the editor's own, takes an `Operate` for any
  of them only if the account may edit that block, and closes a block in the peer when no
  instance holds it.

## The graph on the plugin protocol

- A plugin asks with `EditorMessage::WatchBlocks { queries }`, a list of `BlockQuery`s. The host
  answers from its mirror with `Blocks { query, blocks }` whenever the answer changes
  (`plugin_host/graph.rs`).
- A `BlockInfo` carries the block's type, author, parent (`BlockLocation`: `Root`, `Detached` or a
  block), name and whether it was named by hand, references, the account's access and the
  artifact source.
- A plugin changes the graph with `CreateBlock { block_id, content_type, parent, name, artifact,
  content }`, `SetParent` and `SetName`. The host takes them only when the account may edit the
  block and the parent it names, and hands them to `be::create`, `be::set_parent` and
  `be::set_name`.
- On the plugin side: `editor.watch_blocks(query)` and the `Blocks` handle from `editor.blocks()`.

## Creating, seeding and replacing

### Creating

- A new block's first content travels with it: `CreateBlock` carries the encoded content, and the
  worker creates the block, then writes the content as its first commit, before any later command
  for that block runs.
- `Creation::create` and `Blocks::create` make a block this way, Detached unless a parent is
  named. The host sets the parent of a block made in a creation dialog once it has the id.
- Creating a database makes two blocks: `block_editor_beui::database::create_database` creates a
  schema with a Name field and a database pointing at it, and makes the schema a child of the
  database, so the graph is right before any editor opens it.
- `content_file_creation::<C>` is the creation view that makes a block of a file type from the
  file the user picks.

### Seeding

`editor.seed_content(block, &content)` sends `EditorMessage::SeedContent` for an existing block.
The peer writes it as the first commit, and ignores it for a block that already has content or is
open, so it cannot overwrite anything.

### Replacing

`editor.replace_content(block, &content)` sends `EditorMessage::ReplaceContent`; the host takes it
only for a block the account may edit.

- The worker hands it to the open session's `Live::replace`, or saves it straight to the server
  when nothing has the block open.
- `Live::replace` publishes the new content as a commit on the head it knows, retrying on the
  newer head a rejection names, so the bytes go through the chunked upload, not the session.
- An owner that replaces settles on that commit and tells followers to reload.
- A follower sends the owner `SessionMessage::Replaced { head }`; the owner reloads from that
  head, reapplies what it had sequenced but not sealed, and seals, so a header edit made during
  the replace is not lost.
- Users: the image, audio and PDF editors' "replace" buttons and the pixel art export's
  regeneration.

### Dynamic artifacts

They reach content from the artifact session through the host: `EditorHost::content_of` gives a
regeneration a projection of its source, and `EditorHost::replace_content` writes the result.
Compiled logic and the pixel art export regenerate this way.

## Children and duplication

- Moving a block into a container, deleting a child or replacing it with a copy asks the
  container's content to change. `PluginEditor` sends that to `be::change_child`, and the type
  answers with `Root::child_edit` (`guides/blocks/adding_a_block.md`, "Children"): a presentation
  adds or drops a slide, a database clears or rewrites the cells that link the block, a hotbar
  unpins or repoints a component.
- If the container is not open, `change_child` holds it and answers `None`, which callers treat as
  "not yet" and retry.
- `be::duplicate` adds the copy to the mirror with the source's type, references and metadata
  (without its artifact source), and the worker copies what the source's session shows, or its
  head when it is not open, into the copy's id before anything opens the copy.

## Undo

Undo lives in the app's peer, not in an editor, because the peer is the one place that sees every
edit to a block from this device, whichever editor made it.

- A type registered with `kind_with_history` gets a session with an undo and a redo stack. Every
  operation an editor sends records a step; a step arriving within 750 ms of the last may be
  absorbed into it.
- `be::undo` and `be::redo` turn a step into operations the session applies like any other edit.
- Plugins send `BlockCommand::Undo` and `BlockCommand::Redo`, and watch whether either is possible
  with `EditorMessage::WatchHistory`, which the host answers with `HistoryStates` when they change.
- The text editor keeps its own undo (see "Editors with their own model").

## Blocks the app reads itself

The app reads some blocks itself, not only through an editor.

- `be::hold` opens a block for the app and keeps it open when the last editor showing it closes,
  because `be::close` leaves a held block alone. Nothing releases a held block before the stack
  stops, which is when the workspace changes.
- `sync_ui_settings` takes the theme from the UI settings.
- `sync_local_settings` hands:
  - the input settings (keymap, key repeat, and pointer settings for every pointer with
    per-device overrides) to the `--session` seat through `beui_adapter_drm::InputControl` and to
    the nested Wayland clients' keyboard;
  - the display settings (a mode per monitor, keyed by its EDID make, model and serial, or by its
    connector when it has no EDID) to the seat's outputs through
    `beui_adapter_drm::DisplayControl`, which switches mode live.
- Every setting is an `Option`; unset means the default. For a pointer setting that is libinput's
  own, which the seat reports per device and an editor reads as the `InputDevices` host value. For
  a monitor it is the preferred resolution at the fastest refresh rate offered there, reported
  with the monitor's modes as the `Displays` host value (`guides/plugins/editor_api.md` covers
  host values).
- The seat runs before anyone signs in, so the app keeps a copy of the last applied settings of
  each kind in `app_state` and applies it at startup. The block stays the source of truth.
- A kind the app applies itself implements `local_settings::LocalSettings` and is kept by a
  `SettingsSync`; its `apply` answers whether the settings may be saved now.

### Keeping display settings

Display settings may not be saved until someone keeps them. `display::guard` opens the host's
"Keep these display settings?" prompt (`styled::KeepChanges`, a card on every screen) for a change
that moves what a connected monitor shows, or for a monitor started at a fastest rate other than
its preferred mode that it was never kept at. It is the host's because it must work before any
shell plugin runs.

- Each change is a round (`Guard::prompt`). Another change while asking starts a new round and a
  full countdown, and an answer counts only for the round it was given in.
- Keep saves, storing a kept default as the monitor's mode.
- Revert, Escape, or 15 seconds of the frame clock (counted by the guard itself) put back the
  earlier modes, or the preferred mode for an untried default.
- `SettingsSync::commit` writes those edits into the block too, holding them until its content is
  loaded and reading the block with them on top until a revision holds them.

## Settings, settings profiles and sessions

### The Settings block

Each account keeps what is its own in a workspace under one top-level `Settings` block it
authored.

- Every reader finds it by author: an administrator can see every block, including other accounts'
  `Settings`, and nothing may pick those up. `root_settings::own_settings_blocks` (plugins) and
  `RootSettings::owned` (block-app) list the account's own.
- When two devices each made one before seeing the other's, both are read as one
  (`Settings::merged`, the lowest id winning a clash) and written through the lowest.
- The settings editor shows someone else's `Settings` or settings profile with a notice, and
  makes and changes nothing in it.

### Layers

Each kind of setting (`UiSettings`, `InputSettings`, `DisplaySettings`, `EditorSettings`, the
logic hotbar) is a block of its own, read through layers:

1. The built-in default.
2. The `Settings` block's `kinds`, mapping a kind's content type to the block that holds it for
   every device.
3. A settings profile: a `SettingsProfile` block, a child of `Settings` listed in its `profiles`,
   with `kinds` of its own. Each device (`device_profiles`, keyed by client id) uses one, and
   block-app makes one named after the device the first time a device opens the workspace.

- `Settings::layers(kind, profile)` lists a kind's blocks from the bottom, and `be_block::layered`
  folds their contents with the kind's `Layered::over`: what the profile sets wins field by field,
  what it leaves unset falls through. Every field of a layered kind is an `Option` for this.
- `Editor::inherited_settings` gives a kind's editor what the layers below its block resolve to;
  the UI, input and display settings editors show it for what is unset.
- `Editor::effective_settings` is what every layer resolves to. The workspace reads
  `EditorSettings` this way: the default editor of each block type, keyed by the editor ids the
  host's catalog lists (`guides/plugins/editor_api.md`).
- The hotbar is not merged: the logic editors use the topmost layer that has one, and make one for
  every device when there is none (`RootSetting::ensure`).

### Sessions

A session is what a person has open, separate from the settings, so starting a new one keeps
them.

- It is an `EditorView` of a shell editor: workspace-ui (`WORKSPACE_EDITOR`), or linux-desktop
  (`LINUX_DESKTOP_EDITOR`) for a client run as the desktop with `block-app --session`.
- `Settings::sessions` lists every session; `Settings::session(shell, client)` is the one this
  client uses for that shell, so the desktop and the app each keep their own while sharing the
  device's settings profile.
- The host opens the shell on that session, creating one named after the device when the client
  has none, and reopens the shell when it changes.
- block-app's sessions screen (the workspace list's dropdown, or Sessions and profiles in the app
  menu) picks the device's settings profile and session, or starts a new one of either.
- A shell keeps its dock layout and recents in the session's state. Every tab, and the file tree,
  is an `EditorView` child of the session that the layout references; closing a tab detaches its
  view.
- A block whose type is one of `profile::VIEW_EDITORS` is its own view, so a workspace session
  opened on the desktop keeps its tabs in its own session, and closing it leaves that session
  alone.

## View state

What a person has open is blocks too, of one generic type: `EditorView { editor, content, state }`
names the editor that draws it, the block it shows (none for the file tree or the workspace), and
a `LatestMap` of opaque `ViewState { bytes, refs }` the editor owns.

- `refs` are the blocks an entry points at. `references()` is `content` plus every entry's refs. A
  deleted or replaced child is blanked or repointed in place, so the bytes can index into `refs`.
- An editor learns its view block from `Open { view_block }`, which a parent sets with
  `ChildTarget::viewed_by(view)`.
- `editor.view_state(key)` and `editor.set_view_state(key, state)` read and write it. Writes skip
  the block's editability, since view state is the viewer's own.
- An editor on no content (the file tree, the workspace) is opened on its own view block, and the
  host links that block's content as an `EditorView`. `be_block::profile::VIEW_EDITORS` lists the
  editor ids that are not content types.

Both shells are built from `crates/block-shell`, which holds the tabs, their views, the saved
layout, picks, dialogs, host panels and program windows. workspace-ui lays them out as Files beside
a pane of tabs; linux-desktop floats every block in its own window over an empty main pane, with a
bar along the bottom for the app menu, the sessions to open and the time.

## Keys

Each workspace has a random content key (`Config::content_key`) that seals its content, commits,
block metadata and session traffic. The server never holds it in the clear. `crates/be-keys`
holds the cryptography.

- **On the device.** The app keeps each workspace's key in its app state, per account
  (`AppStateStore::workspace_key`). A workspace with no key there opens on the unlock screen
  (`crates/block-app/src/keys.rs`, `ui/keys.rs`) instead of starting the peer.
- **Recovery.** Each account has a twelve-word recovery phrase, shown once and checked by typing
  three of its words. It derives an X25519 key pair; the server stores only the public half
  (`SetRecoveryKey`). Each member's copy of a workspace key is sealed to it (`PutWorkspaceKey`,
  `GetKeys`), so a new device opens a workspace with the phrase alone.
- **A new phrase** (the workspace menu's New recovery phrase) must reseal every key the old one
  sealed, which the server checks, so it can only be made on a device holding all of them.
- **Sealing for members.** On connect, the worker seals the workspace key for every member with a
  recovery key and no sealed copy (`ListMemberKeys`), covering the creator and anyone invited
  since. The server only inserts a sealed key, never replaces one, so a member cannot overwrite
  another's.
- **Adding a device.** The new device shows an eight-character code. The user types it on a device
  that holds the key and has any workspace of the account open (the worker gets every key the
  device holds). The two run SPAKE2 keyed by the code over the server's relay (`Pair`, delivered
  as `Paired` to the account's other connections), and the open device seals the key under the
  agreed secret. The server would have to guess the code to read or replace it, and gets one
  guess: a wrong code ends the request.

## Presence

Presence is what a peer shows others while it is there (a cursor, a selection). It is never
saved.

### In the session

- `SessionMessage::Presence { kind, value }` carries one kind of it to everyone in the session,
  sealed like every session message.
- `Live::set_presence` shows a value (`None` takes it away) and sends only when it changed.
  `Live::presence` is what the other peers show, keyed by `ClientId` and kind. A peer never sees
  its own.
- When `SessionChanged` names a participant it has not seen, a peer shows that participant what
  it is showing, so a late joiner sees everyone at once. When a participant leaves, what it showed
  is dropped.

### In the app and plugins

- The worker publishes each block's peers into `Shared::presence` when a session says they
  changed, and the host passes them to instances holding that block as
  `EditorMessage::PeerPresence`.
- A plugin shows presence with `EditorMessage::ShowPresence`, which the host takes from any
  instance that holds the block and may view it, and takes away when the instance closes.
- Plugin side: `editor.show(Some(&value))`, and `editor.peers::<P>()`, a signal of every peer's
  value of one `PresenceKind` (`be_block::presence`).
- The framework shows a `UserActive` value of its own whenever the host says the block is visible,
  with a colour from `pick_free_color` against the colours already showing.
- The text editor's cursors and the canvas's pointers and selections work the same way, each with
  its own colour picked the same way rather than tied to the peer's `UserActive` entry.

## Editors with their own model

The text editor cannot hand its state to a projection: `text_editor_core` wants a `Document` that
hands out anchors, so a cursor stays on its character while others type.

- An anchor names one byte and dies with it. `text_editor_core::AnchorTable` makes anchors on
  demand only for bytes something holds a position on, and moves them on every edit, so its cost
  follows the cursors and undo steps, not the text's length.
- `text_block::document::BlockDocument` keeps the bytes and their table itself. It turns each
  local edit into `TextOp::Delete` and `TextOp::Insert`, which the editor pushes into its
  `ContentProjection` every frame.
- It adopts a change from elsewhere by diffing the projection's text against its own: anchors on
  unchanged bytes move with them, the rest are dropped.
- Its undo is its own, because it must give the text core back its cursors. Each step remembers
  the anchors either side of the text it replaced and both versions of that text, so undo finds
  the text wherever it moved and skips it when someone else changed it since.
