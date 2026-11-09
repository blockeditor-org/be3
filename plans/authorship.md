# Authorship

Builds on plans/chat.md step 1a, which defined the bytes a message signs (`crates/be-type-chat/src/signed.rs`).

Signing what a person writes, so nobody with Edit can post as someone else.
Chat is the first user. Not implemented yet.

## What exists

- `Commit::author` is whatever the client wrote. A session names participants
  by connection (`client-id`), and presence says who a connection is. Anyone
  holding a block's data key can write anything, including as someone else.
  The server records when it received each commit (`block_commits.received`).
- A recovery phrase derives an X25519 key pair. The escrow key also derives an
  Ed25519 signing key through HKDF (`EscrowSecret::sign` in
  `crates/be-block-keys/src/lib.rs`). Contacts verify each other's identity
  keys out of band (guides/blocks/keys.md).

## The design

- **The signing key** is an Ed25519 key derived from the identity secret
  through HKDF, as `EscrowSecret` derives its signing key, and its public half
  is added to `ContactCard` and to key changes (`seal_key_change`), so a
  contact verifies it the way it verifies the identity key. Chosen over XEdDSA
  with the identity key itself: XEdDSA would change no format, but has no
  well-reviewed Rust implementation, while Ed25519 is already in the tree. No
  format is frozen, so adding the field costs nothing. Record it as a decision.
- **Host calls.** `sign(bytes)` and `verify(account, bytes, signature)`
  returning verified, unverified key, or bad. Apps never hold the identity
  secret. Both are calls on the app protocol, with no framework types.
- **What is signed:** everything a member authors (messages, edits,
  deletions, reactions). A chat message signs chat id, message id, author,
  claimed time, body hash, reply, and the hash of the last message the author
  saw (both hashes defined and stored by plans/chat.md step 1a). That last
  one proves a message was written after it, and shows a gap when one of an
  author's messages is removed.
- **Open in what 1a signs** (`crates/be-type-chat/src/signed.rs`): taking a
  reaction back removes its key (a `LatestMap` entry with no value), which
  carries no signature, so anyone with Edit can remove another's reaction
  unnoticed; and an edit's signed bytes name the message but not its hash nor
  the edit it replaces, so an older signed edit by the same author can be
  replayed over a newer one. Bind each edit to the message's hash and a
  sequence number (or the previous edit's hash), and sign removals as records
  with no value, before signing ships.
- **Members who aren't contacts** have keys only from the server: trust them on
  first use, keeping the pinned key in the refs database beside contacts,
  marked unverified; show them differently from contacts, and warn when the
  key changes.
- **Timestamps.** A signed time is only what the author claims. The server
  stamps each relayed operation with when it received it (plans/chat.md,
  "Arrival time"), so a message carries an arrival time no member chose. The
  stamp is stored in content, which a dishonest session owner could alter;
  accepted.
- Signatures detect forgery; they don't stop someone with Edit rewriting or
  removing messages (plans/deferred/chat_moderation.md).

## Order of work

1. The signing key in `ContactCard` and key changes, and the decision. Done: a
   contact verifies another's signing key; a new recovery phrase carries it
   through a key change.
2. `sign` and `verify` host calls with their `be-sdk-test` doubles. Done:
   verified, unverified key and bad each tested.
3. Chat signs and verifies messages, and shows arrival times. Done: a forged
   message shows as unverified; a painting of each state.
