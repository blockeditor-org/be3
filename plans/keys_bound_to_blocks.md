# Keys bound to their blocks

Depends on: none. Fixes items 1, 2 and 3 of plans/crypto_audit.md; touches
plans/signed_access.md where noted.

Today a device seals to whatever account `KeyBook::contact` answers (the
server's own, too), trusts any key wrapped under a key it trusts (the
settings key, which the server holds, too), names any held key as a block's
current one, and opens any object with any key it holds. So the server alone
can get keys sealed to itself, plant a key an administrator writes under, and
show one block's content, name, relay or notification as another's. The rule
this plan holds the device to: **a key is used only for the block it is a key
of, sealed only to accounts that may hold it, and a key the server holds
vouches for nothing**.

## 1. Two lookups: who sealed it, and whom to seal to

`KeyBook::contact` answers both, and its server special case leaks into
sealing. Split it:

- `KeyBook::sender(account)`: the public key whose seal this device opens:
  itself, a verified contact, or the server (only for the settings block,
  untrusted, as `accepts` already limits).
- `KeyBook::recipient(account)`: the public key this device seals to: itself
  or a verified contact, never the server, even if the server names itself
  as a contact's or this account's id.

Built (fix/never-seal-to-the-server, 948703c94): `KeyBook::contact` answers
only this account and its verified contacts, so `make_escrow` and every other
seal skip the server whatever account it names itself as; `KeyBook::sender`
adds the server for opening, which `accepts` limits to the settings key, and a
contact or this account takes precedence over a server naming itself as them.
Key work's `unsealed` loop seals through `KeyBook::recipient`, which allows the
server only the settings block's scope key, judged by the block this device
minted or opened that key as, not the server's row: a new epoch of the
settings key reaches the server that way after a rotation, and the server
reads the policies through it. `share_with_server` stays the explicit seal.
Test: `the_server_naming_itself_in_key_work_is_sealed_nothing` (fails before
the fix: the workspace's scope key is sealed to the server).

## 2. A key the server holds vouches for nothing

The server holds the settings block's scope key (sealed to it through
`recipient` and `share_with_server`) and so every key wrapped under it: the
settings block's data keys and its children's, the organization's policies
block among them (be-app-settings makes it under settings, not as a scope of
its own). Today an administrator's device trusts that scope key, so a row the
server wraps under it, labelled as any block, opens trusted.

A device keeps a mark of its own, **server-held**, on:

- every key of the settings block, and every key it seals to the server;
- every key it opened only through a server-held key, transitively.

A server-held key opens (the settings subtree must stay readable to every
member and administrator, and writable by administrators), but it is never
trusted for anything outside that subtree: it vouches for no other key (a
wrap under it opens untrusted), and `sealing_key` uses it only for a block in
the settings subtree. Inside the subtree it may seal, since the server reads
those blocks by design; that is what lets an administrator who did not mint
the policies key still write policies. The subtree is the server's graph's
word, so the server can call any block part of it: what that gets it is
showing forged content of that block to readers, never a device writing a
block outside it under a key the server knows. That integrity gap is the same
as the anonymous escrow wraps' and goes to plans/signed_access.md with them.

A viewer of block A colluding with the server could wrap a key it chose under
A's scope key, labelled as block B. That is "the server and one contact can
read a block" (plans/signed_access.md), whose signed roots and records bind
which key may wrap which; the graph's scope relation would buy nothing here,
since the server tells it. keys.md says so.

Tests: `a_key_wrapped_under_the_settings_key_seals_no_other_block` (an
administrator who did not mint the settings key, a data key of block X with a
high epoch wrapped under the settings scope key: X is still written under its
own key); `members_and_administrators_read_and_write_the_policies` (a member
reads, and a second administrator writes, the policies block under settings).

## 3. Objects are opened only with a key of their block

- **The keyring knows each key's block.** `Keyring` keeps `(ContentKey, block)`
  per id, from minting or from the `CarriedKey` it opened with (bound inside
  the seal, so the server can't relabel it; `KeyBook::opened` already checks
  it against the row). A key whose block is unknown is not inserted. Every key
  a device holds has one: an escrow key's is the organization (`mint_escrow`,
  and the `CarriedKey` sealed to each administrator); the server's seal of the
  settings key carries the settings block, which `opened` checks; own-sealed
  pins and recovery material are not ring keys.
- **Checked where the server points, inherited along hashes.** The server
  chooses only what it points at: a block's head (the commit its summary
  names), its metadata, relays, notifications, journal entries and the current
  key. Each of those reads names its block and refuses a key that is not that
  block's (`Keyring::open(block, sealed)`, `StoreError::WrongBlock`, read as
  `ClientError::Forged`): `Peer::open_metadata` the summary's block, the head
  commit `CommitStore::get` its block, `Peer::unseal` the session's block,
  the journal its block, and `open_notification` the notification's block,
  checked before decrypting. Everything reached from a checked object by hash
  (a commit's parents and manifest, nodes, chunks, a repository's tree entries
  and its upstream history) is bound by that hash, the hash of its sealed
  bytes, so the server can't swap it, and opens with whichever key sealed it.
  Version control keeps reading snapshot entries sealed under the working
  blocks' keys, and fork, pull and push history under the upstream's, as it
  does today: those are access questions (a repository's reader needs those
  keys), not forgery, and need no change.
- **A block's current key is one of its keys.** Built (fix/keys-bound-to-blocks,
  8f2ea3f9e): `Peer::sealing_key(block)` refuses a `summary.key` that
  `KeyBook::block_of` does not place in `block` (the server could name block
  A's key as B's, and the device wrote B under a key A's viewers hold).
- **Associated data.** Each seal authenticates the header (`be3K` and key id)
  and the block of the key that sealed it, which the keyring records; opening
  uses the key's own block, so reads along hashes across blocks still open,
  and a header whose key id was relabelled fails. The layout stays; old
  objects no longer open, and no format is frozen.
- **A replica reaches only what it was handed.** Built: a replica's
  `read-part`, and the parts its `publish` and `stash` reuse, are confined to
  the parts it loaded and put and to blocks those are sealed in
  (guides/type_modules.md, What a replica reaches). Not the plan's first
  framing (a reused part naming its block): a module that serves a block can
  copy its bytes anyway, and `read-part` was the wider reach.
- **Escrow secrets leave the content keyring.** Built: `KeyBook` holds them
  apart (guides/blocks/keys.md, Escrow), so no object whose header names one
  opens with it; they are X25519 secrets, never AEAD keys.
- **The nonce.** With the format changing anyway: XChaCha20-Poly1305, its
  192-bit nonce derived from a nonce key (HKDF of the content key,
  `be3.vault.nonce.v1`) over the block id and the plaintext, so collisions
  stop mattering and the content key is no longer hashed directly. Objects that
  are never deduplicated (relays, notifications, metadata, journal entries)
  take a random nonce instead, so the server can't see two equal ones
  (crypto_audit, simplicity 5). Associated data per block also scopes
  deduplication to a block, which keys already did; decisions/deterministic_seal_nonce.md
  is updated to say which seal which.
- **One symmetric seal.** The audit found three formats (the vault's, the
  random-nonce `seal_with` in be-block-keys, backups'). This ends with one:
  `be_block_keys::aead`, XChaCha20-Poly1305 with a 24-byte nonce and associated
  data, `seal(key, nonce, associated, plain)` and `open`. The vault (which
  then depends on be-block-keys) puts its `be3K` header and the block id in
  the associated data and derives the nonce or draws it; be-block-keys' own
  seals (`seal_with`: sealing to an account, own seals, invitations, pairing)
  and be-server's backups draw it and bind their context there. One format,
  three callers that differ only in what they bind.

Order, each piece landing on its own with its tests: section 1 (built); the
current key (built); the pointer reads naming their block (built); a
replica's reach (built); escrow secrets moving out of the keyring (built);
then section 2 with the associated data, nonce and one
seal format (whose `seal_with` part waits for signed access 3b and 3a, which
build on it).

What stays open: keys a device opens without trust (through escrow, whose
seals are anonymous, or the server's seal of the settings key) still read
their block. Anyone who knows the escrow's public key, the server included,
can seal a made-up key of block X to it and so show administrators forged
content of X. Authenticating escrow wraps (a member's `seal_for` to the escrow
key) closes it; that goes with plans/signed_access.md's administrator records.

## Reads across blocks

An inventory of every read of sealed objects found that nearly every caller
has its block at hand, and that only version control crosses blocks: snapshot
tree entries name manifests sealed under the working blocks' or a checkout's
key, fork, pull and push history is sealed under the upstream's key, merges
read sides from several blocks, and reused parts are resealed from wherever
they were. All of those are reached by hash from an object the server could
not choose, so the rule above leaves them as they are; resealing snapshots
instead would double what version-controlled content stores and make the
"publishes without downloading" of guides/blocks/version_control.md false.
Caches of opened values keyed by hash alone stay as they are; only a cache of
a pointer read (`Peer.whole`, keyed by head) needs the block in its key.

## Tests (each fails today)

be-block-client, with the harness's server database playing the attacker:

1. Built: `the_server_naming_itself_in_key_work_is_sealed_nothing` (the
   server as an unsealed account, and as an administrator due an escrow).
2. `a_key_wrapped_under_the_settings_key_seals_no_other_block` and
   `members_and_administrators_read_and_write_the_policies` (section 2).
3. Built: `another_blocks_commit_served_as_a_head_is_refused` (a head, its
   metadata, a listed revision).
4. Built: `a_relay_sealed_under_another_blocks_key_is_refused`.
5. Built: `a_notification_sealed_under_another_blocks_key_is_refused`.
6. Built: `another_blocks_key_named_as_current_is_refused`.
7. Built: `a_replica_reaches_no_part_of_a_block_it_was_not_handed`
   (be-block-type-host).
8. Built, as be-host-stack's `a_fork_pulls_and_pushes_through_its_upstream`:
   the crossings the rule keeps still read.

## Docs

guides/blocks/keys.md (Trust, sealed bytes, What the server can and cannot
do), guides/blocks/client.md (notifications), the decisions on deterministic
nonces and on the server-sealed settings key, and plans/crypto_audit.md
(items 1-3 fixed, the escrow gap moved to signed_access).
