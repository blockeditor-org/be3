# Lazy rekeying: a reused part keeps the key it was sealed under

Depends on: none.

Set aside by the user after three reviews of a built attempt
(`feat/large-blocks-retention-rekey-2`, not merged): its automatic reseal
through escrow lets a malicious server get made-up content vouched for, and the
design needs a decision first. The search index (plans/search.md) and chat
(guides/chat.md) work without it; they pay for a rotation in full.

## The goal

`Vault::write` reseals every reused part sealed under an older key
(`stale_parts`, `reseal`, the stale-part fetch in `Peer::write_content`), so
after a rotation the next save of a large block (the search index, a chat, a
long blob) re-uploads all of it, and a rekey's forced seal rewrites everything.
Keep a part reused from the same block under the epoch that sealed it, and seal
with the current key only what a save writes new, so a save costs in proportion
to its change, rotation or not. A head then mixes the block's epochs, and a
reader needs each.

Done, as first planned: after a rotation, saving a ten-part block that changed
one part uploads one part and its nodes; a viewer with only the new epoch's
wrap still reads every part; a block moved into another scope and then saved
stays readable to the new scope's members.

## The design the attempt reached

- `Vault::write` keeps a `Part::Reused` as it is. A part reused from another
  block (a version-control checkout filled from another's snapshot, a merge
  over parts reusing its sides') is sealed again under this block's key
  (`Vault::adopt`, `foreign_parts`, called from `Peer::write_content`), opening
  it only with keys this device trusts, so no block needs another block's key
  and nothing held only through escrow is copied as this device's write. A part
  the block's head already lists is its own without fetching it (a single-chunk
  part's key is only in its own header).
- A rotation rewraps every data key the device trusts under the new scope key;
  a move into another scope (`Peer::rescope`) wraps every trusted epoch not yet
  wrapped under the destination, refreshing the subtree's keys first, and keeps
  the fast path for a move within a scope.
- The forced seal after a rekey publishes nothing for a type that hands back
  every unchanged part, and still reseals whole-bytes types.
- For an epoch a block's head uses that the device holds only through escrow (an
  administrator's device, after the member who trusted it left), the attempt
  resealed those parts of the head under the block's new key and published that
  (`Peer::reseal_head`, `Vault::rekey`, `keys_of_part`, `missing_tops`). That
  is the part that failed review, below.
- Its tests are worth keeping: a save after a rotation writes three objects; a
  leaver gets neither the new key nor the old-key objects of the new head; a
  viewer let in after two rotations reads parts under three epochs; a member of
  the scope a block moved into reads it; a checkout shared with another account
  reads without the one it came from; a part reused from another block is
  sealed again, written from a device that never read the source.

## What a design must answer first

### A server can forge content an administrator vouches for

Escrow wraps are anonymous sealed boxes (`EscrowSecret::open`, keys.rs
`unwrap`), and the escrow public key is public. So a server can: mint a content
key K, wrap it to the escrow key and add it as a data key of a member's block;
publish a head sealed under K holding anything (commits are not signed); record
a rotation of the block. An administrator's device rotating it finds the head on
an epoch it holds only through escrow and reseals that content under a key it
trusts, so every member reads server-made content as an administrator's write.
Before, members never read content the server made up, since they hold no key it
chose (guides/blocks/keys.md, What the server can and cannot do). A move or
share by an administrator launders the same way, with a person's action. The
options:

- **Authenticate escrow wraps by sender**, sealed from the wrapping device's
  identity as account seals are, and reseal only epochs escrowed by a verified
  member of the organization; such keys become a trust level of their own.
- **Reseal only on an explicit administrator action** that shows what it will
  vouch for, never inside `tend`.
- **Never vouch**: leave such epochs unwrapped and accept that people let in
  later cannot read those parts. The attempt rejected this, but it is the only
  option with no new trust.

Whichever is chosen, keys.md's "What the server can and cannot do" must say it.

### A reseal must be durable

The attempt cleared the rotation (and, on a move, left the escrow-only epoch
unwrapped under the new scope) before the reseal published. A reseal that then
fails (four rejected writes, a missing key, the network, the process quitting)
leaves the head on an epoch nobody let in later can open, and nothing retries.
Reseal under the minted data key and push its objects before `Rotate` or
`SetParent`, then publish; or persist "heads to take off epoch E" in the refs
database for `tend` to retry; or have the server keep the rotation pending until
the device reports the reseals.

### The old epoch must not come back

After a reseal, any write built on the earlier tree reuses parts under the
escrow-only epoch E again: a member's live session merging after the reseal
rejected its save, an offline outbox, a version-control restore or checkout, a
`be sync` pass. Nobody wraps E under the new scope, so the head closes to people
let in later. Count as the block's own only epochs wrapped under its current
scope key, and adopt (reseal) parts under any other epoch, so a trusting device
reseals E parts on its next write and an escrow-only device gets `Locked`.

### Prefer a device that trusts the keys

Every administrator holds `Manage` on every block, so `KeyWorkChanged` reaches
them all, and an administrator's device that succeeds races the member's, which
could simply rewrap E. The administrator's then downloads and reseals every head
in the workspace on the member's epochs. Have the server offer a rotation to an
escrow-only device only when no member device that trusts the keys exists, or
none has tended within some time, or have such devices defer.

### Rotations nobody can do, and their signals

- Refusing on any untrusted key (the attempt's first fix) makes routine
  rotations a sticky sync error on every administrator device (`tend` records
  it; `sync_state` shows `SyncState::Error` until reconnect), and leaves a
  rotation pending forever when the only trusting device is gone, while the
  block's key stays one the removed person holds.
- Refusing on epochs the head no longer uses blocks a rotation forever, since
  rewriting adds an epoch but never removes the old row. Check only the epochs
  the head uses (`keys_of_part` needs no key to say which).
- Dropping `Locked` silently (the attempt's second fix) hides a rotation every
  device lacks keys for, and a half-done one whose reseal failed. Treat only a
  `Locked` before the RPC as another device's, log it, and give administrators
  a non-error notice of rotations waiting for a device that holds their keys,
  or let the server report rotations pending past some time.
- An epoch the head uses that this device lacks altogether was skipped, so the
  rotation completed without it although another device could have wrapped it.
  Return `Locked` then, to leave the rotation to a device that holds it.

### What it gives up, stated where people read it

- A removed member who kept an old key can open every part of the current head
  no save has rewritten, not only history up to the removal, if a colluding
  server serves the objects.
- A device that has not learned of a rotation (or that a colluding server keeps
  unaware) seals new content under the old key, and later saves reuse it; neither
  the device nor the server checks a publish's key against the newest epoch.
- Parts under an epoch no device holds any more stay unreadable to anyone let in
  later.
- Record a decision, since this reverses part of #804, and update
  guides/blocks/server.md ("a subtree is always sealed under one key") and
  guides/blocks/keys.md ("reseals the parts it reuses").

### Costs

- Computing a block's own parts from its head on every `write_content` walks
  and decrypts the head's tree (a `summary` round trip when the device knows no
  head, attempted offline too). Do it lazily: only for reused tops missing from
  the store, and never when the parts hold no `Part::Reused`.
- `Vault::sealed_under`'s cache should hold only what a write looks up; walking
  a whole head for its keys fills it for every node.
- A move between scopes walks the subtree one listing per block, and inspecting
  heads for their epochs fetches every head's nodes.

### Smaller points the reviews raised

- A `PartRef::Chunks` part is checked by its top object only, on the
  assumption that one write sealed all its chunks under one key; a forged one
  mixing keys must fail cleanly (`MissingKey`), not half-reseal.
- A reseal's epochs were computed before the rotation's RPC, so an escrow-only
  epoch a concurrent write introduces is missed.
- A test of the forced seal must wait for its answer (the attempt counted
  answered forced seals, `Replicas::rekeys_sealed`), or "the head did not move"
  passes before it ran.
- Moves must refresh the subtree's keys before wrapping (as rotations do) and
  refuse a block whose current key has no row, or unknown epochs pass silently.
- keys.md must not keep two paragraphs on what a move rewraps, and its server
  trust list must say a reused part extends the exposure to the current head.

### Tests a design needs

Beyond the attempt's (above): a server-made escrow-wrapped key on a member's
block (a row and wrap added through the server's database) whose content no
member must come to read as vouched; a reseal that fails or is contended after
the rotation; a live member device writing or merging after a reseal; `adopt`
refusing a part under an escrow-only key; an untrusted epoch the head does not
use, skipped without a reseal; an epoch the device lacks; a move within a scope
adding no key rows; store-level tests of `Vault::rekey`, `keys_of_part` and
`missing_tops`; and an escrow-only administrator's effect on a member's settings
rotation asserted, not only `tend` succeeding.
