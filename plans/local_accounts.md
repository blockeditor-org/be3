# Local accounts

Depends on: none.

A local account should be the device's offline-first clone of an account, with no server behind
it, as a git repository works with no remote.

Today a local account (a local profile, guides/hosting.md) is an account on a be-server that
be-host embeds:

- `be-host-stack/src/embedded_server.rs` serves it on a port, started by `HostCore::new` on every
  native run, whether or not anyone uses a local account.
- It brings be-server's store, hyper and s3s into be-host's native build.
- It keeps its own objects under the data directory's `server/`, beside the client's store, so a
  local account's objects are on disk twice.
- The web has no embedded server, so it has no local accounts.

## Principles

- **Gaps are offline-first gaps.** Most of what a local account lacks, a server account lacks
  too while it is offline. Fill those gaps in the client, for both.
- **Move logic to the client rather than implementing it twice.** Where a local account needs
  something the server does today, move it into the client (be-block-client, be-host-stack) and
  have the server check or store what the client did. The server keeps only what needs an
  authority across devices or accounts: sign-in, access between members, sessions shared between
  devices, quotas and notifications.
- **No server code in the client.** be-server's crates stay out of be-host's dependencies. The
  tests that need a real server, for sync, sharing and several devices, start one as a test
  dependency (be-host-stack's and be-host-testing's `Harness`).

## What the client already does without a server

- `Peer::cached` builds a peer with no network. Offline, summaries, listings and `read_contents`
  answer from the refs database, and content opens at `Peer::held_head`
  (guides/blocks/client.md, The refs database).
- A block opened offline gets a made-up session the device owns (`Open::made_up`), and a publish
  becomes a local commit in the outbox (guides/blocks/host.md, Disconnects, offline and
  stopping).
- Creating, defining, moving and duplicating a block are kept on the device and wait in the
  outbox, online as offline.
- Search indexes are built and searched on the client (guides/blocks/host.md, Search).
- Keys are made on the client, and key rows are cached in the refs database, so a cached peer
  unwraps offline (guides/blocks/keys.md).

## Gaps

Each says whether a server account offline has the same gap.

1. **The local head is final.** With no server, a local commit is the head, and nothing waits to
   be confirmed. The made-up offline session already behaves this way; a local account stays in
   it. Local accounts only.
2. **Version control.** Filled for server accounts offline: every operation runs against the
   device's store and waits in the outbox, and a branch someone else moved meanwhile merges as
   the repository's divergence, keeping both commits (guides/blocks/version_control.md,
   Operations).
3. **Making the account, its organization and its roots.** The server makes a local profile
   (`create-local-profile`), puts it in the personal organization and answers the roots and
   settings a host looks for at sign-in. A local account makes its ids, roots and settings on the
   device. A server account offline already makes workspaces: a root's creation waits in the
   outbox like any other block's (be-block-client's
   `a_workspace_made_offline_reaches_the_server_with_its_keys`). Making the account and its
   organization is local accounts only.
4. **Keys.** The server stores key rows and records rotations (`GetKeys`, `rotations`). A local
   account keeps its rows on the device only. A server account offline mints a new block's keys
   on the device and sends them with its creation through the outbox. Rotation and escrow run
   only on key work the server hands out (`Peer::tend`), after a member is removed, so they
   never start offline; a local account has one member and nothing to rotate. Local accounts
   only.
5. **The graph.** The server works out access and the `size` and `tree_size` it lists. A local
   account's one member has everything (`Refs::local_listing`). Sizes are not counted on the
   device yet: a local account's blocks list a size of 0, so the files show none. Fill it by
   counting the bytes of the objects each block holds as its commits settle, and each tree's
   from its children's.
6. **History and reclaiming storage.** `fetch_history` reads history from the server, and holds,
   retention and reclamation run there (guides/blocks/server.md, Liveness and reclamation). A
   local account holds all its history, and reclaims with the same storage crates on the device.
   Offline, a server account reads the history the device holds (`Peer::history`, filled);
   holds, retention and reclamation stay the server's, so the rest is local accounts only.
7. **What only a server can do:** sharing, access lists, members, invites, pairing a device,
   block agents, push and contacts. These are refused with "needs a server", as sharing, leaving
   and approving a device already are offline. A local account's menus still offer Share and
   Access, which it refuses; they should not be offered.

On the web a local account lives only in the origin's browser storage: clearing site data, a
private window or the browser evicting it under pressure loses the account and everything in it,
with no phrase to recover it. Making one there says so; asking the browser to keep it
(`navigator.storage.persist()`) is not done yet.

What the server enforces that a local account goes without, its one member holding every
block: `may_create`'s reserved My files and Unfiled ids and who may make workspaces (the host
makes those roots under their reserved ids itself), and access and visibility. A block agent's
scoping (`bound_may`) is the server's, and a local account makes no agents: they are refused
offline. The app sandbox is the host's own (guides/apps/host_services.md), so it holds the same.

## Steps

1. Sizes on the device (gap 5).
2. Delete `embedded_server.rs` and be-server from be-host-stack's dependencies; tests keep
   be-server as a test dependency. Update guides/hosting.md (Local profiles and the default
   server), guides/blocks/server.md (Running it, Tests), guides/blocks/host.md and
   guides/running/native.md.

Later: adding a server to a local account, by sending everything it holds, as pushing to a new
remote.
