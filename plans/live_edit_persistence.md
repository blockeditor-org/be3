# Saving live edits less often

Edits are durable in a journal of operations, commits are written at pauses,
and the server prunes as commits arrive (built). What is left: tie the
journal's format to the module tag, and restart long-lived sessions so their
state stays small. The canvas has the same problem as chat (plans/chat.md).
Making each commit cheaper is plans/large_blocks.md.

## What exists

- **The journal** (guides/blocks/client.md, "The journal"; guides/type_modules.md,
  Peers). The owner appends every operation it sequences to the block's journal
  on the server and in the device's refs database: with others in the session
  in the same call that relays it (`sessions.append`), alone folded every
  `APPEND_INTERVAL`. A journal has a base (commit, sequence number, session
  state) and is tagged with the module that wrote it. Each commit rebases it,
  dropping every entry. A replica opening on its base replays it; a reader
  outside the session (`Modules::readable`: export, `be download`, the
  tracker) replays it when its tag is the module's.
- **Commits at pauses.** `QUIET_INTERVAL` (30 s) with no edits,
  `COMMIT_INTERVAL` (5 min) after the first, `JOURNAL_BYTES` (1 MiB) appended,
  on close, at once when a peer of another build greets the owner or a
  takeover leaves it owing, and locally when the connection drops
  (`crates/be-sdk-type-guest/src/replica.rs`, `Replicas::disconnected`).
- **Session state** (tombstones, text positions, split fragments) is saved only
  as a journal's base. A follower joining with the journal's base and
  numbering greets with them (`Catchup { base, sealed }`) and the owner's
  `Snapshot` carries no state; otherwise the owner hands it over as before.
  Nothing restarts a session, so a block someone always has open keeps all its
  tombstones.
- **Compatibility.** Each type module has a `Tag` (its hash), and an owner
  answers a different tag with `Incompatible`, so two builds of a type never
  edit one session. The server refuses an append to a journal of another tag
  until the owner rebases; a replica of another tag opens on the head alone
  and starts the journal over at its first append or commit, losing the other
  build's entries without saying so (3).
- **Retention.** The server prunes as each publish arrives (guides/blocks/server.md):
  `RetentionPolicy::default()` keeps one state per minute for the last hour,
  then one per 10 minutes for a day, per hour for a week, per day for 30 days
  and per week after that; the head, bookmarks, merges' parents and whatever
  is inside the organization's `history_kept_for` stay. A device merging an
  offline edit falls back to the remote head it built on when pruning cut the
  chain between them (`Peer::reconcile`). Loading a commit gives every replica
  the same positions and ids, so replicas that load one commit can exchange
  operations with no `Snapshot`.
- **Undo** is local to each device (`Undo` in `be-block`).
- `decisions/editors_write_every_change_as_it_happens.md` puts batching in the
  client, which is where this plan puts it.

## The design

### Left from the journal and commits at pauses

- A follower that takes over keeps the operations it applied since its last
  sealed head and appends them again after its rebase. One that cannot number
  them (it missed some, or was catching up) commits at once instead, so they
  live only in its memory for one round trip.
- The server has no notice that it is closing for a new build. Clients notice
  when they reconnect: a refused build updates the host, which flushes first,
  and on any disconnect the host commits what each replica owns locally,
  since a reconnect reopens replicas from their heads.
- A module restart reopens its replicas from their heads and replays what the
  host kept since the last commit; it does not read the journal, which holds
  the same operations.
- `be sync` reads heads without the journal, since it records which head it
  synced; search does not read contents yet.
- The base's session state is sent whole at every commit; restarts (5) bound
  it.

### 3. Live-edit formats stay unfrozen

- Journal entries and base state are tied to the module tag. Before its code
  changes, an owner commits, and the commit goes through the outbox, whose
  stored form already handles format changes.
- A journal no module can replay is dropped and the block marked, so the editor
  says that edits from an older version were lost. That happens only when an
  owner crashed and the type changed before anyone with the same tag recovered
  the journal.

### 5. Session restarts

- A restart is rare and triggered by cost, not a timer: when session state is
  over 1 MiB and larger than the visible content, at the next commit while no
  gesture is open, preferably while the owner is alone; and for free when
  everyone leaves. A restart forced in the middle of a session is a sign the
  limit is too low.
- The owner commits and drops session state, and every replica reloads from
  that commit. Loading a commit gives the same positions and ids on every
  replica (all content from client `LOADED` at offsets `0..len`, list slots in
  stored order; items stored without an id get one derived from their place),
  so this needs nothing new from loading.
- The visible content is the same before and after, so anything in visible
  offsets carries over: cursors and selections in presence, and a follower's
  pending edit, which it re-sends as the visible splices `SeqOp` reported. Only
  an insert concurrent with someone deleting around it loses `Delete`'s
  guarantee, and only for edits in flight.
- **Undo stops at a restart.** Each undo step records its session. Undo with an
  older step next does nothing, and shows "Can't undo past here" with a button
  that opens version history at that point. Redo stops the same way.
- Record restarts and the undo boundary in `decisions/` when built.

## Order of work

3, then 5.

## Tests

- A journal written by another module tag is dropped and the block marked.
- A restart keeps a follower's pending edit and the peers' cursors; a painting
  shows the message when undo reaches it.
