# Offline-first

A plan to keep blocks on the device so they open and edit without a
connection, chosen per block tree as yes, lazy or no. Each step says what
"done" looks like. Items marked **(ask)** need a decision from the user during
the work.

On web there is no service worker yet, so the app itself still needs the
network to load. Offline-first there means the data is kept: a tab that loses
its connection keeps editing what it has, and unsent edits survive closing it.

## Where it stands

- What the server keeps is frozen (`guides/frozen_formats.md`). A local
  account's blocks live in the embedded server, so on native they already
  work offline.
- For a remote account, nothing on the device is frozen and most of it is not
  kept:
  - `app.sqlite3` (accounts, identity secrets, `client_id`, copies of the
    input and display settings) is deleted and recreated when its schema does
    not match (`block-app/src/app_state/native.rs`). That also makes a new
    `client_id`, which orphans the user's this-device settings layer.
  - `be-objects/` holds sealed objects in the frozen format, but
    `forget_caches_of_other_builds` (`block-app/src/main.rs`) deletes it on
    every new build.
  - Heads (`Peer::heads`), the graph mirror (`be/graph.rs`) and keys
    (`KeyBook`) live only in memory, so cached objects cannot be read offline.
  - `Peer::signed_in` needs a connection before the peer exists, and
    `Live::open` asks the server for the head.
  - `Live::seal` saves and publishes in one step and only the session's owner
    may seal, so nothing can be sealed offline; operations not yet sealed are
    lost on disconnect, and queued graph commands live in an in-memory channel.
  - On web the peer keeps objects in a `MemoryStore`, `ObjectStore` is
    synchronous, and nothing in the repository uses IndexedDB.
- `Settings`, `SettingsProfile` and every settings kind are unfrozen, and a
  settings block the app cannot read is reset without asking (`be::hold`).

## Modes

The choice follows the viewer: it lives in the user's this-device layer as a
map from block to mode, and a block takes the mode of its nearest ancestor
with an entry. Blocks shared with the user resolve the same way, from the
user's own settings.

- **Yes.** The whole subtree is fetched in the background and kept current.
- **Lazy** (the default). What has been opened is kept. Removing old content
  under a size limit comes later.
- **No.** Content is kept in memory only. Unsent edits are still written to
  disk until the server has them.

A local account and the settings tree itself are always yes.

## Unsent edits are commits, never operations

Live operations and session state are not frozen and will keep changing, so
nothing that waits to be sent is stored as an operation. Unsent work is a
sealed local commit: the block's full persistent form, chunked, so a small
edit adds a chunk or two. Sealing already happens every `SEAL_INTERVAL`;
offline, the peer seals locally instead of publishing, and a crash loses at
most that interval, as it does online today.

That keeps unsent edits across an upgrade for every block type, frozen or
not:
- Publishing a commit never decodes it, so a fast-forward pushes it as it is.
- When the server's head diverged and the new build can decode both sides,
  `be_session::resume` merges them as it does today.
- When it cannot decode the local side (an unfrozen type that changed), the
  local commit is kept with `Peer::stash` and shown as a conflict, and the
  block takes the server's head. Nothing is dropped.

The graph changes made offline (create, rename, move, delete) are kept the
same way: as their own frozen record in the refs database (below), not as
protocol messages, since the protocol is not frozen and an old build cannot
talk to a new server.

## 1. Land the settings break first

Land section 1 of `plans/settings.md` with its one server wipe before step 5
turns this on: a wipe would strand unsent edits held on devices. Steps 2 to 4
can go on alongside it.

`Settings` and `SettingsProfile` keep their shape: nothing in
`plans/settings.md` needs more than additions to them. A block's or an
organization's settings is a new type, and tags, the prefix registry and the
offline choice are new kinds.

## 2. Freeze `app.sqlite3`

Independent of the rest, and it fixes the orphaned this-device layer today.

- Only add to its schema, as the server does, and drop the recreate path.
  `client_id` never changes once made.
- The web store (`app_state/web.rs`, localStorage) gets `#[serde(default)]` on
  every stored struct and keeps its keys.
- **(ask)** Whether the input, display and remapper copies stay in it or go
  once settings blocks are kept locally; they exist so a setting applies
  before the peer starts.
- Done: the client database is a frozen format in `guides/frozen_formats.md`,
  with a test that opens one written today.

## 3. A local-first peer

Tracker issue BE-DZM7WR, phase 5 of `plans/daily_driver.md`. Nothing here is
frozen yet, and until step 5 the app keeps deleting its caches on a new build.

1. **A refs database** per account and organization (SQLite on native): each
   block's local head, last-known remote head and `clean_at`; the graph mirror
   with its metadata; key rows as the server sent them, still wrapped, opened
   with the identity secret; the outbox of graph changes; and which blocks are
   kept, with local reference counts for the objects they hold.
2. **A peer without a connection.** The peer opens from the refs database and
   the object store, and a connection attaches later and comes and goes.
3. **Open and edit from the local head.** Opening reads the local head; joining
   the session and fetching the remote head happen in the background. A peer
   with no connection is the owner of its own copy and seals local commits. On
   reconnect `resume` / `Live::reconcile` decide between fast-forward,
   publish and merge.
4. **Offline graph changes.** Creating a block needs its parent's scope key,
   so kept scope keys come from the refs database. Sharing stays online only.
5. **Connection.** Request timeouts, pings, reconnect with backoff and on
   network change, and seal on going to the background.
6. **Status.** Offline (n changes waiting), syncing, saved or error, and
   `status().unsealed` counts local commits and the outbox, so the close and
   account-switch guard tells the truth.
7. Done: tests with a transport that drops, stalls and half-opens connections
   on demand: edit offline, restart, reconnect; two devices edit offline and
   both reconnect; the connection dies mid-seal; a local commit of a type the
   new build cannot decode is stashed, not lost.

## 4. The setting

1. **An offline settings kind** in the this-device layer: the block-to-mode
   map, resolved by nearest ancestor from the local graph mirror, so it works
   offline. The organization's policy (required, allowed or blocked;
   `plans/settings.md` step 16) is added later as another layer.
2. **Keeping and letting go.** Yes subscribes to the subtree and fetches the
   objects of every head in it; lazy keeps what was opened; no uses a memory
   store over the file store and writes only unsent commits, which are
   released once published.
3. **UI.** Set it from the block menu and in the settings editor; a kept or
   kept-subtree icon from `beui_core::icons` in the file tree, with a painting.
4. **Losing access.** Unsent edits move to a local unsent area, stashed as
   `Peer::stash` keeps the losing side of a merge, and are offered back if
   access returns. Nothing is deleted automatically for a personal account.

## 5. Freeze and turn it on

1. Freeze the refs database and `be-objects/` (stop deleting it on a new
   build; `plugin-cache` may still go), and the `Settings` root,
   `SettingsProfile` and the offline settings kind (`Root::FROZEN`, a
   `frozen_*.rs` module each). A settings block the app cannot read never
   causes anything to be let go of.
2. A client snapshot in `crates/be-frozen`: a data directory written by a real
   peer, holding unsent commits, outbox entries and kept trees. Its test opens
   it with no server, then connects and checks everything arrives.
3. Update `guides/frozen_formats.md`, then make lazy the default.

## 6. Web

Needs what does not exist yet: an `ObjectStore` over IndexedDB, which is
asynchronous, so an in-memory front with write-behind (and a way for `flush`
to wait on it), and the refs database in IndexedDB too, since localStorage is
too small. Until then web behaves as no. Ask the browser for persistent
storage (`navigator.storage.persist()`) when the user picks yes.

## Later

- Lazy lets go of what has not been opened in a while, above a size limit.
- A service worker, so the web app opens with no connection.
- Protecting the identity secret on the device (BE-SN4EFJ): keeping more
  content makes a lost device worth more.
