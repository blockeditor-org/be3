# One symmetric seal

Every symmetric seal is `be_block_keys::aead`: XChaCha20-Poly1305 with a 24-byte
nonce and associated data. Its callers differ only in what they bind and how
they choose the nonce:

- the vault (`be_block_store::Vault`): its header (`be3K` and the key id) and
  the block the key is of, which the keyring records beside each key, so a
  relabelled header or a key placed in another block opens nothing; a derived
  or a random nonce (decisions/deterministic_seal_nonce.md);
- `be_block_keys`' seals to accounts, own seals, invitations and pairing: their
  context, with any ephemeral public key that precedes the ciphertext; a random
  nonce;
- be-server's backups: their magic; a random nonce.

## Why

The crypto audit found three formats (the vault's, `be_block_keys`' and the
backups'), each ChaCha20-Poly1305 with a 12-byte nonce and no associated data.
One seal is one thing to review, and binding the context in the associated data
rather than only in a derived key makes a seal of one kind useless as another.
The 192-bit nonce lets a random one be drawn without counting.

## Rejected

- Keeping ChaCha20-Poly1305: a random 96-bit nonce wears out after about 2^32
  seals under one key, which content keys can approach.
