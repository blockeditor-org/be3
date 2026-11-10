# Keys and contacts

Part of the block stack (guides/the_new_block_stack.md). Every block is sealed
with keys of its own, wrapped by the keys of the scope it lies in; the server
holds only wrapped keys. `crates/be-block-keys` holds the account-level
cryptography, `be_block_store::Keyring` the content keys, and
`be_block_client::KeyBook` (one per `Peer`) the rows and their unwrapping.
Design records: `decisions/deterministic_seal_nonce.md`,
`decisions/a_server_sealed_key_never_seals_anything.md`,
`decisions/workspace_keys_without_device_key_pairs.md`.

## Keys

- **Sealed bytes name their key**: `b"be3K"`, the 16-byte key id, a nonce
  derived from key and plaintext, then ChaCha20Poly1305 ciphertext. This covers
  chunks, commits, manifests, metadata, session traffic and wrapped keys. An
  unknown id is `StoreError::MissingKey`.
- **Data keys (DEKs)**: one per block, made at creation (`CreateBlock` carries
  it; `BlockSummary::key` is the current one). A block keeps every epoch, so its
  history stays readable.
- **Scope keys (KEKs)**: a scope root is a root block, a detached block, an
  owner-only block (guides/settings.md, Owner only), or a block with grants of
  its own (`BlockSummary::scope`, `BlockGraph::scope`). A
  DEK is wrapped under its scope's KEK, a scope root's KEK under its parent
  scope's, and a KEK is sealed to each account that may open it with that
  account's identity key.
- **Trust.** A key sealed to an account names its sender and is sealed by it
  (`RecoverySecret::seal_for`/`open_from`, binding both public keys, both
  account ids and the key id). A device takes such a key only from itself or a
  contact it verified (`KeyBook::contact`) and silently ignores any other row,
  including one the server sealed to its public key. Keys made here, sealed by a
  verified sender, or wrapped under such keys are *trusted*
  (`KeyBook::trusted`); keys opened only through escrow, or the server's seal
  of the organization settings key, are not. A device reads with any key it
  holds but seals, wraps or hands over (`seal_to`, `wrap`, `escrow_wrap`,
  `Peer::sealing_key`, `Peer::offer`) only with trusted ones, so a key the
  server made up never protects anything new.
- **On the server**: tables `keys` (id, block, kind `data`/`scope`/`escrow`,
  epoch) and `wrapped_keys` (key, `wrapped_by`: a key id or `account:{uuid}`,
  sender set by the server to the storing account). `GetKeys` serves rows of
  blocks the account can view and wraps by its account or a key it is served;
  `PutKeys`, `CreateBlock` and `SetParent` store rows only for blocks the
  caller may edit, never replace a sealed row, and refuse a seal to any account
  but the caller's own and its contacts'.
- **On the client**: `Peer::signed_in` loads every row (kept in the refs
  database, so `Peer::cached` unwraps offline) and unwraps what it can
  (`KeyBook::absorb`). A read meeting an unknown key refetches once, then fails
  with `ClientError::Locked`; `Peer::learn` fetches rows for blocks that appear
  later.
- **Moving and sharing.** `Peer::set_parent` into another scope rewraps the
  moved DEKs (old epochs too) under the destination KEK, gives each block a
  fresh DEK and rewraps scope roots inside, all in one `SetParent`. A move
  within a scope or to Recently Deleted changes no keys. `Peer::share` grants
  and makes the block a scope root sealed to the grantee's verified key.

### Rotation

When an account stops being able to view a block (revoked, left, moved away,
contact or member removed), the server records the block in `rotations`, drops
that account's wraps and ends its sessions there. `GetKeyWork` tells a client
what it can do: rotations on blocks it may edit, scope keys not yet sealed to
someone who should have them (`unsealed`, whose list is `keys::sealed_to`), and
scope keys not yet escrowed. `Peer::tend` does all of it; the worker calls it
on connect, `ContactsChanged`, `KeyWorkChanged` and after its own changes.

`Peer::rotate` sends one `Rotate` for a subtree: new epochs of every scope key,
every data key rewrapped, a new data key per block. The server takes it only
whole. Content is not resealed, except that the owner of a live session on a
rekeyed block seals once more (`Session::rekey`), which reseals the parts it
reuses under the new key (`Vault::write`). A rotation nobody can do yet
waits until a client that may edit the block tends. A kind open inside
(guides/settings.md) is also offered by any viewer to its own contacts among
the viewers, since the owner's editors may know none of them.

Not built yet: a move into another scope lists the subtree a level at a time,
and old data keys of a block this device cannot open are not rewrapped, so
that history stays closed to the new scope.

### Escrow (company organizations)

The escrow key (`be_block_keys::EscrowSecret`) is both an X25519 key and,
through HKDF, an Ed25519 signing key: a key row of kind `escrow` on the
organization id, sealed to each administrator. The first administrator device
to open the organization makes it (`CreateEscrow`), and `mint_scope` seals
every new scope key to it too, so administrators open everything. Demoting or
removing an administrator records an escrow rotation: the next epoch is signed
by the current one over `EscrowKey::chained`.

An owner-only block is never escrowed
(decisions/owner_only_settings_are_never_escrowed.md): its scope key
has a version-8 id (`be_block_protocol::owner_only_key`), bound into every seal
of it so a server cannot relabel it, and is sealed to its author alone, not wrapped under the scope above nor sealed to the escrow key
(`KeyBook::mint_own_scope`; `escrow_wrap` and `Peer`'s moves skip it), and
`store_keys` refuses any other wrap of it. Key work never lists it as
unescrowed, a rotation by anyone else leaves it out (it needs nothing from the
scope above), and the author's own rotation mints it again the same way. A new
device of the owner opens it through the identity secret, and a new phrase
reseals it with everything else the account sealed for itself.

A member wraps only under an escrow key it pinned from somewhere the server
cannot forge: its own, the invitation (`InviteCode::seal_escrow`), an agent's
maker, or a secret an administrator contact sealed to it. It follows the chain
one signature at a time (`keys::verify_escrow`) and pins the newest verified
(`PinEscrow`); an unverifiable newer key is not used, and the host's menu shows
`KeyBook::escrow_warning`.

### Identity, new phrases and devices

- **Identity**: a twelve-word recovery phrase derives an X25519 key pair; the
  server stores the public half (`SetRecoveryKey`). A device keeps the secret
  (`HostStateStore::identity`); without it, or with one that does not match,
  the account opens on the unlock screen (`be-host-stack/src/keys.rs`,
  `ui/keys.rs`). A local account (`ServerLocation::Local`) is never asked to
  save its phrase: its secret sits beside its database anyway, so be-host saves
  the first phrase without the check screen, as the dev workspace does.
- **A new phrase** must reseal every row sealed to the account by itself or a
  verified contact, and everything it sealed for itself (`OwnSealed`), which
  the server checks, so only a device with the old secret can make one. Each
  verified contact gets a key change (`contacts::new_identity`,
  `RecoverySecret::seal_key_change`, kept in `key_changes`); a contact's device
  follows changes one at a time from the key it holds, reseals what that
  account sealed to it (`ResealKeys`) and replaces its row (`PutContact`).
- **Inviting**: the only way into an organization, and it hands over no key.
  The inviting device makes an `InviteCode` (200 bits) and names the invitation
  by `InviteCode::lookup`, so the link is `{server}/#invite/{CODE}`
  (`invite_link.rs`). The invitation carries the inviter's card and escrow key
  sealed with the code; accepting returns the invitee's card like a contact
  link. The web build takes the code from the address bar and replaces it with
  `#`; the server never sees what follows `#`.
- **Adding a device**: the new device shows an eight-character code, typed on a
  device of the same account; the two run SPAKE2 over the server's relay
  (`Pair`, `Paired`), and the open device seals the identity secret under the
  agreed key. A wrong code ends the request, so the server gets one guess.

## Contacts

Every sharing relationship starts with a link passed outside the host, and a
device seals a key only to a contact's public key verified that way.

- A **contact card** (`be_block_keys::ContactCard`: account id, identity public
  key, name, email) is sealed with a link's code (`ContactCard::seal`, keyed by
  code and side). A contact link is `{server}/#contact/{CODE}`. What an
  account keeps for itself (cards, link codes) is sealed with a key derived
  from its identity secret (`RecoverySecret::seal_own`): sealing to the public
  key would let the server forge a row the owner trusts.
- **The exchange** (`be_block_client::contacts`): the maker posts
  `CreateContactLink`; the other side reads it, opens the card, and on
  "Connect with NAME (EMAIL)?" accepts with its own card. The server records
  both `contacts` rows and sends `ContactsChanged`; the maker's device then
  opens the accepter's card, writes its row and deletes the link
  (`finish_links`, on connect and on `ContactsChanged`). Invitations finish
  the same way.
- **A pending grant**: a link may carry a block, an access level and the
  block's scope key sealed with the code (`Peer::offer`). Accepting applies the
  grant at once, so a share works while the maker is offline; finishing
  reseals the scope (`Peer::seal_share`). A grant to a contact whose key the
  granter has not verified yet works on the server at once and is sealed later
  through `unsealed`.
- `RemoveContact` deletes both rows and revokes each one's grants on blocks
  the other may edit, in every shared organization. Contacts are per account,
  across organizations; a grant also needs membership.
- **In the host**: the Contacts dialog, the share dialog (whose "Invite someone
  new" calls host-services' `make-contact-link`), and the worker's
  `Shared::contacts`, the only keys a share is sealed to.

## What the server can and cannot do

It stores and relays everything, so it can withhold, delay, replay or delete
rows, refuse requests, and see who shares with whom, block shapes, sizes and
timing. It cannot read content or metadata, and it cannot choose a key a device
will seal with: keys sealed to an account are sender-authenticated and taken
only from verified contacts; a contact's key changes only by a change sealed
with the verified key; cards, codes, pending grants and invitation escrow keys
are sealed with codes it never sees; a device wraps only under keys it trusts;
escrow keys are used only when pinned or signed down from a pinned one.

It still decides, and devices trust it for: which held key is a block's current
data key and which epoch is newest (so a server colluding with someone who lost
access, or a demoted administrator, can keep a device writing under a key that
person knows); grants, membership and roles (a device seals to any verified
contact the server lists as granted, and the escrow secret to any verified
contact it lists as an administrator); which blocks an account knows exist;
and the graph's shape, sizes and timings.
