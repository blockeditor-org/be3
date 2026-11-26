# Escrow epochs are signed by their maker's identity

An escrow key is an X25519 key and nothing else. Each epoch is signed by the
administrator who made it, with the account signing key
(`RecoverySecret::sign_escrow`), over `EscrowKey::chained` in the escrow chain's
own domain, which be-block-keys holds with every other domain that key signs in.

## Why

Epochs used to carry an Ed25519 key of their own, derived from the escrow
secret, which signed the next epoch. Once epochs were bound to administrator
records in the organization's log (decisions/key_epochs_name_their_heads_inside_the_key.md),
the maker's own key signs them and the escrow's signing key signed nothing: a
field, a column and a derivation kept for no use, and a key a demoted
administrator still held.

## Rejected

- Keeping the escrow signing key beside the maker's signature: two signatures
  that must agree are two things to check, and the escrow's proves nothing a
  demoted administrator could not also make.
