# The block stack

Every block lives in the `be-*` crates. The server (`be-server`) is one more
peer that never decrypts; the host is a peer of its own
(`crates/be-host-stack`); every editor keeps its content here. To add a block
type, read guides/adding_a_block.md first. This guide and the files under
`guides/blocks/` are for work on the stack itself and on how the host holds it.

## Goals

1. Durable state is bytes: a block's persistent form is a byte string, stored
   as chunks. Backups are a commit chain over those bytes.
2. Live editing converges through a session owner, not a CRDT.
3. Offline divergence resolves by three-way merge over the persistent bytes,
   so a wholesale rewrite conflicts visibly instead of interleaving.
4. Partial reads: a ten gigabyte video streams; it is not loaded to be shown.
5. The server never sees content. It holds objects, refs, a graph and
   accounts, and understands only the last three.
6. Reclamation is reference counting, not tracing.

Why the stack is split this way: `decisions/block_stack_separates_durable_bytes.md`.

## The layers

```
be-block-store     objects, chunking, encryption         client + server
be-block-commit    commit DAG, retention, merge          client + server
be-block-session   ownership lease, sequencer, resume    client + server
be-block-graph     parents, edges, access, refcounts     server (and host mirror)
be-block-protocol  the wire format (WIT be3:server)      client + server
be-block-keys      account-level cryptography            client + server
be-block-model     objects with ids, merge, undo         client
be-lib-sequence    the ordered sequence under text/lists client
be-lib-search      the full-text index, kept as parts     client (guides/blocks/host.md, Search)
be-block           content traits and content types      client
be-block-vcs       snapshot trees, diff, tree merge      client
be-block-client    a peer: local store, refs db, relay   client
be-block-sync      a folder <-> a directory on disk      client (native, guides/sync.md)
be-server          the always-online peer                server
be-server-bin      the executable, with its portal       server
be-host-stack      the host's peer, worker and mirror    host
```

Block types run in type modules, each holding a replica per open block
(be-block-type-host on the host side, be-sdk-type-guest in the module):
guides/type_modules.md.

## The parts

- guides/blocks/server.md: the storage crates (store, commit, session, graph),
  be-server (transport, protocol, publishing, organizations, access,
  announcements, accounts), running a server, and the test harnesses.
- guides/blocks/client.md: `Peer` and `Connection` (transport, liveness, the
  fault relay), reading and writing, live sessions and ownership, the refs
  database, saving and the outbox.
- guides/blocks/content_types.md: the content traits, `be_block_model` and its
  field kinds, sequences, the stored form and how it changes, blobs, and
  testing a type. ("Adding a content type".)
- guides/blocks/host.md: how the host holds content: the type registry, stale
  blocks, the worker and its replicas, connections, offline and stopping, the
  graph mirror, metadata, creating, importing, exporting, children and undo.
- guides/blocks/apps.md: blocks on the app protocol: content frames and
  `ContentProjection`, state and change callbacks, the region driver, the graph
  queries and commands, presence, peer messages and `LiveText`.
- guides/blocks/version_control.md: repositories, checkouts, snapshots,
  conflicts as data, scopes and local ids.
- guides/blocks/keys.md: keys and their scopes, rotation, escrow, identity and
  pairing, contacts, and what the server can and cannot do.
- guides/blocks/workspaces.md: organizations, workspaces and the files pane,
  device profiles and settings the host applies (block settings are
  guides/settings.md), sessions and view state.

Which formats are frozen (none yet): guides/frozen_formats.md.

## Not built yet

- **Authorship.** Nothing vouches for who wrote an item: the deterministic game
  stores each move's account as a field the sender fills in. Signing operations
  is not enough, since the session owner sequences and relays everything; the
  stored items would have to carry their own signatures.
- **Previews from content.** Server-side previews are impossible under
  end-to-end encryption. Search is a client-built index per root block
  (guides/blocks/host.md, "Search").
- **Direct peer connections.** Sessions relay through the server.
- Gaps specific to one area are listed in its part.
