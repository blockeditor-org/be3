# Sealing derives its nonce from the key and the plaintext

`be_block_store::Vault` and everything sealed for content (chunks, commits,
manifests, metadata, session traffic, wrapped keys) use a 12-byte nonce derived
from the key and the plaintext rather than a random one.

## Why

Objects are content-addressed by the hash of their ciphertext. A deterministic
nonce makes identical content under one key seal to identical bytes, so it
deduplicates, while content under different keys shares nothing.

## Rejected

- A random nonce: identical content would no longer deduplicate. Replacing the
  nonce means replacing the deduplication scheme too.
