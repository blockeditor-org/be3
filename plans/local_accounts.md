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
2. **Version control.** Snapshots, branches, combining, bringing in, undo and the rest
   (`be-host-stack/src/version.rs`) run in the served lane, which waits for the outbox to be sent
   and reads and writes the server between steps. A branch moves by compare-and-swap on the
   server. A server account offline can do none of this either. Fill it by running the
   operations against the device's store, moving the branch locally and sending the move later
   through the outbox. A branch someone else moved meanwhile becomes a divergence to merge, as a
   block's head does.
3. **Making the account, its organization and its roots.** The server makes a local profile
   (`create-local-profile`), puts it in the personal organization and answers the roots and
   settings a host looks for at sign-in. A local account makes its ids, roots and settings on the
   device. Check whether a server account offline can make a workspace: whether a root's creation
   waits in the outbox like any other block's.
4. **Keys.** The server stores key rows and records rotations (`GetKeys`, `rotations`). A local
   account keeps its rows on the device only. Check whether making and rotating keys offline is
   kept and sent later on a server account.
5. **The graph.** The server works out access and the `size` and `tree_size` it lists. A local
   account's one member has everything, and sizes are counted on the device. Offline listings
   already answer from summaries.
6. **History and reclaiming storage.** `fetch_history` reads history from the server, and holds,
   retention and reclamation run there (guides/blocks/server.md, Liveness and reclamation). A
   local account holds all its history, and reclaims with the same storage crates on the device.
   Offline, a server account reads only the history the device holds.
7. **What only a server can do:** sharing, access lists, members, invites, pairing a device,
   block agents, push and contacts. These are refused with "needs a server", as sharing, leaving
   and approving a device already are offline.

## Steps

1. Fill the gaps server accounts share (2, 3, 4 and 6 where they apply), moving logic from the
   server into the client.
2. A peer with no server: its local head is final, and its account, roots and settings are made
   on the device.
3. Onboarding's "Create local account" makes one, on the web too. `--dev-workspace` uses one.
4. Delete `embedded_server.rs` and be-server from be-host-stack's dependencies; tests keep
   be-server as a test dependency. Update guides/hosting.md (Local profiles),
   guides/blocks/server.md (Running it, Tests), guides/blocks/host.md and
   guides/running/native.md.

Later: adding a server to a local account, by sending everything it holds, as pushing to a new
remote.
