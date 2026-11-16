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
- **Keys travel with their block and root**: what is sealed or wrapped is
  `be_block_keys::CarriedKey`, the key, the block it is a key of (the
  organization, for an escrow key) and the hash of that block's access log
  root when the sender holds it pinned, so whoever opens it knows which block
  it is for whatever the server's row says (`KeyBook::opened` drops a key
  whose block differs) and may pin the root (below).
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
  including one the server sealed to its public key. It seals a key only to
  itself or a verified contact (`KeyBook::contact`), whatever account key work
  or the member list names, the server's own included; the one key it seals to
  the server is the settings block's scope key, by the block it knows that key
  is of (`KeyBook::recipient`, `Peer::share_with_server`). Keys made here, sealed by a
  verified sender, or wrapped under such keys are *trusted*
  (`KeyBook::trusted`); keys opened only through escrow, or the server's seal
  of the organization settings key, are not. A device reads with any key it
  holds but seals, wraps or hands over (`seal_to`, `wrap`, `escrow_wrap`,
  `Peer::sealing_key`, `Peer::offer`) only with trusted ones, so a key the
  server made up never protects anything new.
- **Keys are bound to blocks where the server points.** Every key a device
  holds has a block (`KeyBook::block_of`, filled by `mint` and `opened` alone).
  What the server chooses for a block is opened only under one of that block's
  keys, or fails with `ClientError::Forged(block)`. That covers the current key
  (`Peer::sealing_key`), a head and its metadata (`Peer::load_head`,
  `metadata_of`), a journal, a relay and a notification (`Peer::open_for`). A
  key the device lacks still fails as missing. Otherwise a server could serve
  what was sealed in an open block as a secret block's, and a reader of both
  would show it there and write back under the secret block's key. Whatever is
  reached by hash from a checked object inherits its binding and is read with
  any held key, which is how version control crosses blocks: snapshot entries,
  history pulled from an upstream, merge sides and reused parts
  (a_fork_pulls_and_pushes_through_its_upstream in be-host-stack). So a new read
  of a commit or a sealed payload by a hash or key the server named takes the
  block it is for and goes through `load_head` or `open_for`, never through
  `load_commit` or the vault directly.
- **On the server**: tables `keys` (id, block, kind `data`/`scope`/`escrow`,
  epoch) and `wrapped_keys` (key, `wrapped_by`: a key id or `account:{uuid}`,
  sender set by the server to the storing account). `GetKeys` serves rows of
  blocks the account can view and wraps by its account or a key it is served,
  a page at a time in the order of key ids (`ServerConfig::key_page_bytes`, each key with its
  wraps), which `Peer::refresh_keys` reads through;
  `PutKeys`, `CreateBlock` and `SetParent` store rows only for blocks the
  caller may edit, never replace a sealed row, and refuse a seal to any account
  but the caller's own and its contacts'.
- **On the client**: `Peer::signed_in` loads every row (kept in the refs
  database, so `Peer::cached` unwraps offline) and unwraps what it can
  (`KeyBook::absorb`). A read meeting an unknown key refetches once, then fails
  with `ClientError::Locked`; `Peer::learn` fetches rows for blocks that appear
  later. A key found missing is not asked for again until keys arrive, so a key
  stored later must be told: a `PutKeys` that stores or changes a row pushes
  `keys-changed` to every other connection of the viewers of each block it gave
  a key or wrapped one of under another key, of each viewer it sealed a key to,
  and of every administrator (all an escrow wrap tells). An agent bound to a
  block may not open it; a device attaching again reads every row anyway.
  `Peer::keys_changed` forgets what was
  found missing, and a `learn` or read asking as keys arrive asks once more.
  The worker then reads its locked nodes again with one `learn`, and
  `Replicas::unlock` opens again each replica found locked that loses nothing by
  it (one with unsealed edits reads again on its next read); a read that keys
  arrived during (`Peer::arrivals`) reads again rather than lock. So a grant may reach
  a device before its key: a share's seal, a pending grant sealed later, or a
  seal key work asks of another editor all unlock it.
- **Moving and sharing.** `Peer::set_parent` into another scope rewraps the
  moved DEKs (old epochs too) under the destination KEK, gives each block a
  fresh DEK and rewraps scope roots inside, all in one `SetParent`. A move
  within a scope or to Recently Deleted changes no keys. `Peer::share` grants
  and makes the block a scope root sealed to the grantee's verified key.

### Access logs

Who may do what is moving from server rows to records members sign
(plans/signed_access.md, decisions/the_server_never_vouches_for_an_identity.md).
Each block has a log of `be_block_keys::Entry`s, each signed with the
signer's account signing key in a domain of its own (`be3.access.v1`) and
naming the hash of the entry before it. The first is the root: the block's
creator and their keys, made by `Peer::creation` (and `import`) and sent with
`CreateBlock`, also through the outbox. A device takes a root's hash only from
somewhere the server cannot write: making the block (`KeyBook::pin_root`,
before its keys are made, which carry it) or a trusted key of the block
(`KeyBook::opened`). Keys of one block that carry different roots leave it
with none (`RootPin::Conflicting`). `Peer::access_log` refuses a log whose
root is not the pinned one, or a block whose root conflicts
(`ClientError::Forged`). A key opened only through escrow or the server's
seal pins nothing, since anyone who knows the escrow's public key can seal to
it; and a device hands a key on with a root only when it holds that root
pinned (`KeyBook::carried_plain`), so an administrator rotating a block it
reaches only through escrow passes on no root at all.

A company organization has a log too, rooted at its creator:
`contacts::create_organization` picks the organization id, signs the root and
sends it with `create-organization`, registering the account's keys first if
the server holds none. What an account pins for an organization is
`contacts::Pinned`, the root's hash and the escrow key, sealed to itself
(`seal_pin`, kept by `pin-escrow` and in the session): the creator pins it on
making it, a member from the invitation, which carries it sealed with the code,
and an agent from its maker. `Peer::organization_log` refuses a log whose root
is not the pinned one. The personal organization has no log.

A device takes a log only checked from its pinned root to its head
(`be_block_keys::check_log`): each entry names the hash of the one before and
is signed by its signer's current key. Every root names its kind
(`LogKind`), so a block's log never checks as an organization's.

### Rotation

When an account stops being able to view a block (revoked, left, moved away,
contact or member removed), the server records the block in `rotations`, drops
that account's wraps and ends its sessions there. `key-work-changed` tells a client
what it can do: rotations on blocks it may edit, scope keys not yet sealed to
someone who should have them (`unsealed`, whose list is `keys::sealed_to`), and
scope keys not yet escrowed. `Peer::tend` does all of it; the worker calls it
on connect, `ContactsChanged`, each `KeyWork` and after its own changes.

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
by the current one over `EscrowKey::chained`. A minted escrow key becomes the
device's escrow (`KeyBook::adopt_escrow`) only once the server holds it, since
the server refuses a key wrapped under one it does not know. A block made under
the old escrow is escrowed under the new one too: by the key work the rotation
reads after adopting it (`unescrowed`) when the server already held the block,
or else once the server takes the block (`Peer::escrow_created`).

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

- **Identity**: a twelve-word recovery phrase derives an X25519 key pair, and
  from its secret an Ed25519 signing key (next section); the server stores both
  public halves (`SetRecoveryKey`). A device keeps the secret
  (`HostStateStore::identity`); without it, or with one that does not match,
  the account opens on the unlock screen (`be-host-stack/src/keys.rs`,
  `ui/keys.rs`). A local account (`ServerLocation::Local`) is never asked to
  save its phrase: its secret sits beside its database anyway, so be-host saves
  the first phrase without the check screen, as the dev workspace does.
- **A new phrase** must reseal every row sealed to the account by itself or a
  verified contact, and everything it sealed for itself (`OwnSealed`), which
  the server checks, so only a device with the old secret can make one. It
  also appends a retirement (`be_block_keys::Retirement`) to every access log
  the account signed in (`recovery-key`'s `logs`): endorsed by the old signing
  key, signed as an entry by the new one, and checked by the server and every
  device, which take nothing the old key signs in that log after it. The
  endorsement is bound to no time or epoch: whoever holds an old phrase can
  retire it in any log where it is still current, at any later moment, so a
  thief who retires first wins those logs (plans/signed_access.md, "Accepted
  limits"). Each
  verified contact gets a key change naming both new public keys
  (`contacts::new_identity`, `RecoverySecret::seal_key_change`, kept in
  `key_changes`); a contact's device
  follows changes one at a time from the key it holds, reseals what that
  account sealed to it (`ResealKeys`) and replaces its row (`PutContact`).
- **Inviting**: the only way into an organization, and it hands over no key.
  An invitation is a link of kind `invitation` (see Contacts below): the
  inviting device makes an `InviteCode` (200 bits) and names the link by
  `InviteCode::lookup`, so it is `{server}/#invite/{CODE}` (`invite_link.rs`).
  It carries the inviter's card and what the inviter pinned for the
  organization (`Pinned`: its access log root and escrow key) sealed with the
  code; accepting it returns the invitee's card as any link does, pins both
  and joins the organization as an editor. It names no email: the link is a bearer
  secret, single use and good for `INVITATION_LIFETIME` (seven days), and the
  server mails nothing, so the inviter sends it themselves. It ends when its
  inviter stops administering the organization: demoting or removing them
  revokes their outstanding ones, and accepting checks their role again. An administrator is
  never made by invitation, since an administrator is sealed the escrow key:
  `set-role` on a member who is already the administrator's verified contact
  does that. The web build takes the code from the address bar and replaces it
  with `#`; the server never sees what follows `#`.
- **Opening an invite link**: be-host reads the invitation before anyone signs
  in (`read-invitation`, a one-shot call, rate-limited per address), which
  answers the organization's name, the inviter's sealed card and where it
  stands (an ended one, its state alone), and opens the card with the code, so a person with no account yet
  sees who invited them to what. The host holds the code (`Joining` in
  be-host's main.rs) while they sign in or register through the portal, then
  looks the invitation up as that account and offers to join. An account that
  can't take it (already a member) is told whose invitation it is and offered
  another account. Reading an invitation marks it opened, so "opened" means a
  holder of the link other than the inviter's own devices read it: be-host
  reads nothing for a code the account signed in made or listed as its own
  (`be_host_stack::is_own_invitation`, keyed by server and account).
- **The invite list** (Invitations, in the organization's menu, for
  administrators; `list-invitations`): every invitation of the organization,
  outstanding and ended (accepted, expired, revoked), with who made it, who
  accepted it and when. A row stays `INVITATION_RETENTION` after it ends, and
  an organization keeps at most `MAX_INVITATIONS_KEPT_PER_ORGANIZATION`; one
  accepted stays until its inviter's device finished it (made the accepter a
  contact), whatever its age. The
  code comes back only to its inviter, sealed to their identity
  (`RecoverySecret::seal_own`), so they copy the link again; any administrator
  revokes an open one (`delete-link`), and an expired one offers a fresh link.
  The Invite member dialog makes a link at once and ends on it, with copy and
  a QR code (`be-host/src/qr.rs`).
- **Adding a device**: the new device shows an eight-character code, typed on a
  device of the same account; the two run SPAKE2 over the server's relay
  (`Pair`, `Paired`), and the open device seals the identity secret under the
  agreed key. A wrong code ends the request, so the server gets one guess.

### Signing what a person writes

What a member authors is signed (plans/authorship.md, chat first;
decisions/an_ed25519_signing_key_beside_the_identity_key.md), so nobody with
Edit can write as someone else.

- **The key**: `RecoverySecret::signing`, Ed25519 derived from the identity
  secret, so every device of the account has it. `sign_authored(block, bytes)`
  signs a domain of its own, the block and the bytes; a signature never
  verifies in another block (`SigningPublic::verifies_authored`).
- **Domain separation**: what the key signs is `be3.authorship.v1`, the 16-byte
  block id, then the bytes, so the prefix is fixed width and the host, not the
  app, chooses it; a signature never verifies in another block than the one the
  editor shows, nor as anything else this key or another key signs (escrow, key
  changes and invitations use other keys or domains). The host also puts the
  asking app's manifest id, framed, ahead of the app's bytes (`sign` and
  `verify`, at most 64 KiB), so an app cannot make a signature that another
  app's `verify` accepts, even in a block both are shown. Apps put their own
  domain first in their bytes (chat's `be3.chat.message`) so their records can't
  be mistaken for each other. `signing-block` names the block signatures bind to, which a record that names its chat or block inside its bytes uses (the block the revision is of, in a revision preview, where `editor.block_id()` is the preview's own). Signatures verify only in the app that made them: another app's `verify` answers `Bad`, which is not a forgery verdict.
- **Apps** never hold it: host-services' `sign` and `verify`
  (guides/apps/host_services.md) answer from the worker (`be-host-stack`
  `authorship.rs`), in the block the editor shows.
- **Verifying** (`Peer::verify_authored`, answering `Signer`): the refs
  database keeps the signing keys known per account (`RefsDb` table `Signers`,
  never forgotten, so what was signed before a new phrase still verifies). A
  contact's key, from its card or a key change, and the account's own are
  verified, the newest being current: an older verified key answers `Changed`,
  since someone holding the old phrase can sign with it. An account with one is checked only against verified keys, after
  rereading the contacts once, so the server cannot sign as a contact. Any other
  member's key comes from the server (`get-signing-keys`: the caller, its
  contacts and the members of the organization open, which in the personal
  organization is every account) and is pinned the first time, unverified; a
  different key the server names later answers `Changed` for what it signed.
  Nothing known that made the signature answers `Bad`; a record found `Bad`
  after asking the server is remembered for that connection (`Forged`), so a
  forged row shown again asks nothing until the device reconnects.

## Contacts

Every sharing relationship starts with a link passed outside the host, and a
device seals a key only to a contact's public key verified that way.

- A **contact card** (`be_block_keys::ContactCard`: account id, identity public
  key, signing key, name, email) is sealed with a link's code (`ContactCard::seal`, keyed by
  code and side). A contact link is `{server}/#contact/{CODE}`. What an
  account keeps for itself (cards, link codes) is sealed with a key derived
  from its identity secret (`RecoverySecret::seal_own`): sealing to the public
  key would let the server forge a row the owner trusts.
- **Links** are one family (`contacts.wit`): a card sealed with the code, a
  lookup id derived from it, and an accept that makes the two accounts
  contacts. Its `link-kind` says what else accepting does: a `contact` link may
  carry a pending grant (below); an `invitation` joins its organization as
  an editor. Any account makes a contact link; only an administrator of the
  organization makes an invitation (`rule` splits `create-link` on the kind),
  and any account opens and accepts it. `list-links` lists the caller's links
  still to act on, outstanding ones included, and `delete-link` revokes one, or
  finishes one accepted; an invitation's row stays for the invite list.
- **The exchange** (`be_block_client::contacts`): the maker posts
  `create-link` (`make_link`, `invite`); the other side reads it
  (`open_link`), opens the card, and on "Connect with NAME (EMAIL)?" (or the
  invitation's join dialog) accepts with its own card (`accept_link`). The
  server records both `contacts` rows and sends `ContactsChanged`; the maker's
  device then opens the accepter's card, writes its row and deletes the link
  (`finish_links`, on connect and on `ContactsChanged`).
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
escrow keys are used only when pinned or signed down from a pinned one. It
cannot sign as a contact: a contact's signing key comes from its card or a key
change.

It still decides, and devices trust it for: which held key is a block's current
data key and which epoch is newest (so a server colluding with someone who lost
access, or a demoted administrator, can keep a device writing under a key that
person knows); grants, membership and roles (a device seals to any verified
contact the server lists as granted, and the escrow secret to any verified
contact it lists as an administrator); which blocks an account knows exist;
the graph's shape, sizes and timings; and the signing key of a member who is
not a contact, taken on first use (a later change shows as `Changed`).

Search does not change what a key protects, and it adds one thing to remember. A
workspace's search index is a block with exactly its workspace's access, so anyone
who can read the workspace can decrypt its index, and the index holds the text of
every block that was indexed. When a block is moved to another workspace, trashed
or emptied, its text stays in the old workspace's index (nothing masks it yet,
plans/deferred/search.md, "Limits accepted"): the interface hides it, but a modified client of
anyone who can read the old workspace could read it. Treat a document that was
once shared in a workspace as read by that workspace's readers, whatever happens to
it afterwards. The server also learns that a workspace has an index, and its size.
