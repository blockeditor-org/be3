# Saving live edits less often

A session owner seals a commit 750 ms after an edit, so a block edited for an
hour makes up to 4,800 commits, and retention keeps every commit from the last
hour. This plan keeps edits durable with a journal of operations, writes commits
only at pauses, prunes as commits arrive, and restarts long-lived sessions so
their state stays small. The canvas has the same problem as chat
(plans/chat.md). Making each commit cheaper is plans/large_blocks.md. Not
implemented yet.

## What exists

- **Sealing.** The owner seals `SEAL_INTERVAL` (750 ms,
  `crates/be-sdk-type-guest/src/replica.rs`) after it starts owing a seal. A
  seal writes a commit and its local head to the refs database in one
  transaction (`Refs::push_publish`), records it in the outbox, and publishes.
- **Relays.** The owner sequences operations and broadcasts them through the
  server (`sessions.relay`). The server sees the sealed relays and keeps
  nothing.
- **Session state** (tombstones, text positions, split fragments) is never
  saved. The owner hands it over in `Snapshot` and `Sealed`
  (`LiveEdit::session_state`), and it goes only when the session ends because
  everyone left. Nothing else restarts a session, so a block someone always has
  open keeps all its tombstones.
- **Compatibility.** Each type module has a `Tag` (its hash), and an owner
  answers a different tag with `Incompatible`, so two builds of a type never
  edit one session.
- **Retention.** `RetentionPolicy::default()` keeps everything from the last
  hour, then one state per 10 minutes for a day, per hour for a week, per day
  for 30 days and per week after that. `retention::plan` uses only each
  commit's id, time and pinned flag, which the server already has.
  `Peer::prune` calls `prune-history`, and nothing calls `Peer::prune`. An
  organization can require every version kept for a while
  (`history_kept_for`).
- **Undo** is local to each device (`Undo` in `be-block`).
- `decisions/editors_write_every_change_as_it_happens.md` puts batching in the
  client, which is where this plan puts it.

## The design

### 1. The journal

- Each operation the owner sequences is appended to a journal on the server:
  sealed as relays are, with its sequence number, lease generation and module
  tag. The server takes entries only from the current owner, for the current
  generation, in order, and deduplicates by `OpId`. One call both broadcasts
  and appends.
- The journal starts from a base: a commit plus the sealed session state as of
  that commit. The server keeps only the newest base state, beside the journal
  and outside history, so retention never sees it. Once the owner reports
  `clean_at` past an entry, the server drops it.
- A new server table is frozen like every other (guides/frozen_formats.md),
  but its payloads are session data a later build may drop.
- On the device, the replica appends to a local journal in the refs database
  instead of writing a local commit per seal, and keeps a local base the same
  way.

### 2. Commits at pauses

- The owner commits when the block has been quiet for a while, when some
  minutes have passed since the last commit with edits, when the journal grows
  past a size, when the last participant leaves, and before its code changes
  (the host stopping or updating, `Incompatible`, the server closing the
  connection for a new build). **(ask)** the quiet time, the longest gap and
  the size.
- **Recovery.** A new owner after a crash, or a follower joining, loads the
  base commit and session state and replays the journal. A joining follower
  then needs no `Snapshot` from the owner.
- **Readers outside the session** (`open`, `read_contents`, `be`, export,
  search) read the head commit, which can be minutes behind while someone
  edits. **(ask)** accept that, or have them replay the journal, which needs
  the type module.

### 3. Live-edit formats stay unfrozen

- Journal entries and base state are tied to the module tag. Before its code
  changes, an owner commits, and the commit goes through the outbox, whose
  stored form already handles format changes.
- A journal no module can replay is dropped and the block marked, so the editor
  says that edits from an older version were lost. That happens only when an
  owner crashed and the type changed before anyone with the same tag recovered
  the journal.

### 4. Pruning as commits arrive

- The server applies retention when a publish arrives: when the new commit
  lands in the same slot as the one before it, the earlier one is dropped in
  the same transaction, unless it is pinned, a merge parent, the head, or
  inside the organization's `history_kept_for`.
- The policy keeps one state per minute for the last hour instead of all of
  them, then the existing buckets.
- A device keeps its last-known remote head and that head's objects locally,
  since it is the base of an offline merge and the server may prune it.
- With the journal, commits already fall at pauses, so this mostly covers
  blocks saved other ways: `write-content`, `be sync`, offline drains.

### 5. Session restarts

- A restart is rare and triggered by cost, not a timer: when session state
  passes a limit (more tombstones than visible content, or a size), preferably
  while the owner is alone and idle, and for free when everyone leaves. It
  waits until no gesture is open.
- The owner commits and drops session state, and every replica reloads from
  that commit. That needs loading a commit to give the same positions on every
  replica (all content from one reserved client id at offsets `0..len`, list
  slots in stored order). Check this first: today `Snapshot` hands session
  state over, which suggests loading doesn't.
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

5's deterministic loading first, since 1 and 5 depend on how a base is
reloaded. Then 1 and 2 together, then 3, 4 and the rest of 5.

## Tests

- An owner cut off by the fault relay after edits and before a commit: the new
  owner recovers every edit from the journal.
- With a manual clock, an hour of steady edits makes a few commits, and pruning
  at publish keeps pinned commits and merge parents.
- A journal written by another module tag is dropped and the block marked.
- A restart keeps a follower's pending edit and the peers' cursors; a painting
  shows the message when undo reaches it.
