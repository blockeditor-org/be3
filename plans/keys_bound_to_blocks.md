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
- **Every read names its block.** The reads that cross blocks today, and how
  each names its block, are listed in "Reads across blocks" below. `Keyring::open(block, sealed)` refuses a key
  that is not that block's (`StoreError::WrongBlock`, which reads as
  `ClientError::Forged`). The vault's reads (`read`, `load`, `read_range`,
  `get_value`, manifests and nodes) and `CommitStore` take the block they read;
  `Peer::open_metadata`, `Peer::unseal` (relays, taking the session's block),
  the journal and `open_notification` (the notification's block) pass theirs.
  Scope keys never seal content, so only data keys open objects.
- **A block's current key is one of its keys.** `Peer::sealing_key(block)`
  refuses a `summary.key` whose row is not `block`'s data key (today the
  server can name block A's key as B's, and the device writes B under a key A's
  viewers hold).
- **Associated data.** Each seal authenticates the header (`be3K` and key id)
  and the block id as associated data, so a sealed object belongs to one block
  by construction as well as by the key check. The layout stays; what changes
  is that old objects no longer open, and no format is frozen.
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

Order: section 1 (built). Then the per-block check of section 3, in two
pieces: first the current key (`sealing_key`) and every read naming its block,
with version control's crossings fixed as above (the larger part: resealing
snapshots, the upstream chain, reused parts naming their block); then section 2
with the associated data, nonce and one seal format, since both change what
trust and opening mean.

What stays open: keys a device opens without trust (through escrow, whose
seals are anonymous, or the server's seal of the settings key) still read
their block. Anyone who knows the escrow's public key, the server included,
can seal a made-up key of block X to it and so show administrators forged
content of X. Authenticating escrow wraps (a member's `seal_for` to the escrow
key) closes it; that goes with plans/signed_access.md's administrator records.

## Reads across blocks

Every read of sealed objects, from the Peer entry points (`read_manifest`,
`load_manifest`, `load_first_part`, `read_part`, `read_span`, `load_commit`,
`open_commit`, `objects_of`, `fetch_history`, `descends`, `merge_base`,
`holds`, `behind`, `metadata_of`, `unseal`, `open_journal`), takes the block it
reads; nearly every caller has it at hand. Those that don't cross blocks:
metadata, relays, the journal, notifications (whose block check moves before
decrypting), revisions (old epochs of the same block), search (an index reads
only its own parts; indexed hashes are identifiers), be-block-sync, the tracker
tools, the server's reads of settings and policies, import (fresh keys),
export, and `duplicate`, which reads the source's plaintext and publishes it
under the copy. Rotation keeps a block's old data keys, so `Vault::reseal`
stays within the block.

What crosses today, all through version control and resealing:

1. **Tree entries** (`be_block_vcs::Entry.content`): a repository's tree is
   sealed under its key, but each entry's manifest under the working block's
   key (version.rs `working`) or a checkout's (`merge_content`), and
   `merge_content`, `Reach::merge`, `merge_over`, `materialize`/`publish`,
   `version_block`, `tree_objects` and `reachable` read them. Fix: reseal the
   changed entries under the repository's key when a snapshot is written
   (`write_snapshot`, which already finds them through `tree_objects`), and
   write merge results under it, so a repository reads only its own key. A
   checkout publishing a snapshot already downloads and reseals across keys
   (guides/blocks/version_control.md's "without downloading" holds only for
   one key, and is corrected).
2. **History across fork, pull and push**: a repository's commits name
   upstream's or a fork's, whose ids are hashes of their sealed bytes, so they
   can't be resealed. Fix: a repository's commit reads accept the keys of the
   repository and its upstream chain (the walks, `CommitStore::get`,
   `is_ancestor`, `common_ancestor` and `MergeBaseWalk`, take the blocks
   allowed), and a repository keeps the upstreams it took commits from even
   after `upstream` changes.
3. **Reused parts** (`Part::Reused`, resealed by `Vault::write` whatever key
   they were under): a part names the block it is from, and the vault refuses
   one from a block not allowed. This also stops a type module's `Publish` or
   `Stash` from resealing another block's part into its own, which it can
   today.
4. **Merge sides** (be-block-type-host `merge`, modules.rs `Kind::load`,
   `Scope`/`PartRead`, version's `Reach::merge`): sides are bare manifests;
   they become `(block, manifest)`, and `Scope` records the block of each part
   it admits.

Keys that are not a block's data keys stay off the per-block open: scope keys
wrap through their own path (`KeyBook::wrap`/`unwrap`), and escrow secrets,
which live in the same keyring as content keys today and so would open an
object whose header names one, move out of it. Caches of opened values keyed
by hash alone (`Peer.whole`, version's commit and tree cache, `Vault.sealed`)
become keyed by block and hash, or they would bypass the check.

## Tests (each fails today)

be-block-client, with the harness's server database playing the attacker:

1. Built: `the_server_naming_itself_in_key_work_is_sealed_nothing` (the
   server as an unsealed account, and as an administrator due an escrow).
2. `a_key_wrapped_under_the_settings_key_seals_no_other_block` and
   `members_and_administrators_read_and_write_the_policies` (section 2).
3. `a_block_shows_nothing_sealed_under_another_blocks_key`: the server serves
   block A's sealed metadata and head as block B's; reading B fails as forged.
4. `a_relay_of_one_block_is_not_applied_to_another`: a relay sealed in A's
   session delivered as B's is refused.
5. `a_notification_sealed_under_another_blocks_key_is_dropped`.
6. `another_blocks_key_named_as_current_is_refused`: the server names A's key
   as B's current key; saving B refuses rather than sealing under A's.

## Docs

guides/blocks/keys.md (Trust, sealed bytes, What the server can and cannot
do), guides/blocks/client.md (notifications), the decisions on deterministic
nonces and on the server-sealed settings key, and plans/crypto_audit.md
(items 1-3 fixed, the escrow gap moved to signed_access).
