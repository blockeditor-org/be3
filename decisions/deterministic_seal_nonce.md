# Content derives its nonce; everything else draws one

`be_block_store::Vault` seals content that deduplicates (chunks, manifest nodes,
commits and other values stored by hash) with a 24-byte nonce derived from what
is sealed: a keyed hash of the key's block and the plaintext, under a nonce key
drawn from the content key by HKDF (`be3.vault.nonce.v1`,
`be_block_keys::aead::derived_nonce`). Everything sealed that never
deduplicates (metadata, relays, journal entries and states, notifications,
wrapped keys) draws a random nonce (`Keyring::seal`).

## Why

Objects are content-addressed by the hash of their ciphertext. A derived nonce
makes identical content under one key seal to identical bytes, so it
deduplicates, while content under different keys shares nothing. Where nothing
deduplicates, a derived nonce only lets the server see that two seals carry
equal plaintexts, so those draw. XChaCha20's 192-bit nonce leaves collisions
out of reach either way (decisions/one_symmetric_seal.md), and the content key
is never hashed directly.

## Rejected

- A random nonce for content: identical content would no longer deduplicate.
- A nonce derived for everything, as before: equal metadata, relays or
  notifications under one key showed as equal to the server.
