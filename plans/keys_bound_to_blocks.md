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

Every seal goes through `recipient`: key work's `unsealed` loop, `make_escrow`
(an administrator the server lists who is the server is skipped),
`seal_share`, `offer`. `share_with_server` is the one seal to the server, and
it keeps its own path: it seals the settings block's scope key and nothing
else (it refuses any other block).

## 2. A key the server holds vouches for nothing

The server holds the settings block's scope key (sealed to it by
`share_with_server`) and so every data key under it. A row wrapped under one
of them must not open as any other block's key, trusted or not: in
`KeyBook::unwrap`, a wrap under a key of the settings block opens only a key
whose row and carried block are the settings block. Nothing in the settings
block legitimately wraps another block's key.

Other blocks: a viewer of block A colluding with the server could likewise
wrap a key it chose under A's scope key, labelled as block B. That is within
"the server and one contact can read a block" (plans/signed_access.md), whose
signed roots and records bind which key may wrap which; this plan leaves it
there and says so in keys.md.

## 3. Objects are opened only with a key of their block

- **The keyring knows each key's block.** `Keyring` keeps `(ContentKey, block)`
  per id, from minting or from the `CarriedKey` it opened with (bound inside
  the seal, so the server can't relabel it; `KeyBook::opened` already checks
  it against the row). A key whose block is unknown is not inserted.
- **Every read names its block.** `Keyring::open(block, sealed)` refuses a key
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
  (crypto_audit, simplicity 5); decisions/deterministic_seal_nonce.md is
  updated to say which seal which.

What stays open: keys a device opens without trust (through escrow, whose
seals are anonymous, or the server's seal of the settings key) still read
their block. Anyone who knows the escrow's public key, the server included,
can seal a made-up key of block X to it and so show administrators forged
content of X. Authenticating escrow wraps (a member's `seal_for` to the escrow
key) closes it; that goes with plans/signed_access.md's administrator records.

## Tests (each fails today)

be-block-client, with the harness's server database playing the attacker:

1. `the_server_listing_itself_as_unsealed_gets_no_key`: key work lists the
   server's account under `unsealed` for a scope key; nothing is sealed to it.
2. `the_server_listed_as_an_administrator_is_sealed_no_escrow`: the server's
   account is listed as an administrator with an escrow rotation due; the
   minted escrow is not sealed to it.
3. `a_key_wrapped_under_the_settings_key_opens_no_other_block`: on an
   administrator's device, a data key of block X with a high epoch, wrapped
   under the settings scope key; it is neither trusted nor opened, and X is
   still written under its own key.
4. `a_block_shows_nothing_sealed_under_another_blocks_key`: the server serves
   block A's sealed metadata and head as block B's; reading B fails as forged.
5. `a_relay_of_one_block_is_not_applied_to_another`: a relay sealed in A's
   session delivered as B's is refused.
6. `a_notification_sealed_under_another_blocks_key_is_dropped`.
7. `another_blocks_key_named_as_current_is_refused`: the server names A's key
   as B's current key; saving B refuses rather than sealing under A's.

## Docs

guides/blocks/keys.md (Trust, sealed bytes, What the server can and cannot
do), guides/blocks/client.md (notifications), the decisions on deterministic
nonces and on the server-sealed settings key, and plans/crypto_audit.md
(items 1-3 fixed, the escrow gap moved to signed_access).
