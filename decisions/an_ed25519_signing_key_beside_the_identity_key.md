# What a person writes is signed with an Ed25519 key derived beside the identity key

An account signs what it authors (plans/authorship.md) with an Ed25519 key derived from
its identity secret through HKDF (`RecoverySecret::signing`, info `be3.identity.signing.v1`),
as `EscrowSecret` derives its signing key. The public half travels wherever the identity key
does and is verified the same way: on the `ContactCard` a contact link carries, and in the key
change (`seal_key_change`) a new recovery phrase sends each contact. The server keeps it beside
the identity key (`recovery_keys.signing`) and names it to members who are not contacts
(`get-signing-keys`), who pin it on first use.

A signature covers a domain of its own and the block it was made in
(`RecoverySecret::sign_authored`), and apps only ask the host to sign (`sign`, `verify` in
host-services): an app never holds the key, and one block's editor cannot make a signature that
verifies in another.

## Why

Ed25519 is already in the tree, well reviewed, and what `EscrowSecret` uses. No format is
frozen (guides/frozen_formats.md), so adding a field to the card, the key change and the
server's row costs nothing.

## Rejected

- XEdDSA, signing with the X25519 identity key itself: it would change no format, but has no
  well-reviewed Rust implementation.
- A signing key the device makes at random and announces: a new device would need it handed
  over like the identity secret, and contacts would need a second verification. Derived from
  the identity secret, every device of the account has it and the key change already carries it.
- Taking a contact's signing key from the server: the server could then sign as anyone. A
  contact's key comes only from its card or a key change sealed with the key verified before, and
  an account with a verified key is never verified by any other (`Peer::verify_authored`).
