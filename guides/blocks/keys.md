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

- **Sealed bytes name their key**: `b"be3K"`, the 16-byte key id, a 24-byte
  nonce, then the ciphertext of `be_block_keys::aead` (XChaCha20-Poly1305, the
  one symmetric seal: decisions/one_symmetric_seal.md), which authenticates the
  header and the block the key is of, as the keyring records it beside each key
  (`Keyring::insert`); so a header relabelled to another key, or the same key
  held as another block's, opens nothing. Content that deduplicates (chunks,
  manifest nodes, commits) derives its nonce from the block and the plaintext;
  metadata, session traffic, journals, notifications and wrapped keys draw one
  (decisions/deterministic_seal_nonce.md). An unknown id is
  `StoreError::MissingKey`. What is sealed to an account, to itself, or with a
  code or a pairing key has no such header: its key is the recipient's, and a
  random nonce leads it (`be_block_keys`).
- **Keys travel with their row, block, root and heads**: what is sealed or
  wrapped is `be_block_keys::CarriedKey`, the key, its own row as its maker
  made it (`Label`: id, kind and epoch), the block it is a key of (the
  organization, for an escrow key), the hash of that block's access log root
  when the sender holds it pinned, and the heads of the logs its epoch was made
  under (`Heads`, below), so whoever opens it knows which key and block it is
  whatever the server's row says (`KeyBook::opened` drops a key whose row or
  block differs: a server cannot serve a wrap of an old key as a new epoch),
  may pin the root, and seals it on only as far as the logs reach those heads.
- **Public keys are checked as they are used**: a seal to or from an X25519
  key whose result is not contributory (a low-order point, which makes it the
  same whatever the secret) is refused (`KeyError::Public`), so a contact or
  an administrator who publishes one gets nothing sealed to it and nothing
  taken from it.
- **Data keys (DEKs)**: one per block, made at creation (`create-block` names
  it and a `put-keys(rows)` in the same write stores it, which the server
  checks at the write's end; `BlockSummary::key` is the current one). A block keeps every epoch, so its
  history stays readable.
- **Scope keys (KEKs)**: a scope root is a root block, a detached block, an
  owner-only block (guides/settings.md, Owner only), or a block with grants of
  its own (`BlockSummary::scope`, `BlockGraph::scope`). A
  DEK is wrapped under its scope's KEK, a scope root's KEK under its parent
  scope's, and a KEK is sealed to each account that may open it with that
  account's identity key.
- **Trust.** A key sealed to an account names its sender and is sealed by it
  (`RecoverySecret::seal_for`/`open_from`, binding both public keys, both
  account ids and the key id). A device takes such a key only from itself, a
  contact it verified (`KeyBook::contact`), or a sender the signed logs vouch
  for: a member the organization's log names, with the keys it names, whom the
  block's log grants Edit or above as of the key's head, or the logs above
  (`KeyBook::vouch`; "Access logs"); it silently ignores any other row,
  including one the server sealed to its public key. It seals a key only to
  itself, to whom the signed logs name, with the keys their record names
  (a grantee its block's log names, an administrator the organization's log
  names: "Sealing", `Peer::sealing_target`), or, for a kind of settings its
  log opens to anyone inside, to a verified contact; never to whom key work or
  the member list alone names, the server's own included. The one key it seals
  to the server is the settings block's scope key, by the block it knows that
  key is of (`KeyBook::recipient`, `Peer::share_with_server`). Keys made here, sealed by a
  verified sender, or wrapped under such keys are *trusted*
  (`KeyBook::trusted`); keys opened only through escrow, or the server's seal
  of the organization settings key, are not. A device reads with any key it
  holds but seals, wraps or hands over (`seal_to`, `wrap`, `escrow_wrap`,
  `Peer::sealing_key`, `Peer::offer`) only with trusted ones, so a key the
  server made up never protects anything new. A key the server holds is
  *server-held* (`KeyBook::server_held`): every key of the settings block,
  which is sealed to the server for it to read the policies, and every key
  opened only through one. It opens, but vouches for nothing (a key wrapped
  under it opens untrusted, even on the device that minted it), and
  `sealing_key` uses it only for the settings block itself, never for a block
  the server's graph places under the settings, since the server can place any
  block there: the settings stay readable, and writable by administrators, and
  the server never gets another block written under a key it knows. So that
  administrators still edit each other's policies, key work lists each
  block's newest data key inside the settings as unsealed for every
  administrator it is not sealed to, and a device that trusts it seals it to
  them as itself. Whatever key work lists, a device never seals a key an
  owner-only scope covers (`KeyBook::owner_only_covers`: the scope key, or a
  key this device minted or opened under one, until a wrap of it under a key
  no owner-only scope covers shows it moved out, `KeyBook::moved_out`; it is
  then sealed and escrowed as any shared key). Nothing moves into an
  owner-only block, so no shared key ever becomes owner-only.
- **Keys are bound to blocks where the server points.** Every key a device
  holds has a block (`KeyBook::block_of`, filled by `mint` and `opened` alone).
  What the server chooses for a block is opened only under one of that block's
  keys, or fails with `ClientError::Forged(block)`. That covers the current key
  (`Peer::sealing_key`); a head or a revision the history lists, and its
  metadata and content (`Peer::load_head`, `metadata_of`, `open_commit`); and a
  journal, a relay and a notification (`Peer::open_for`). A key the device
  lacks still fails as missing. Otherwise a server could serve
  what was sealed in an open block as a secret block's, and a reader of both
  would show it there and write back under the secret block's key. Whatever is
  reached by hash from a checked object inherits its binding and is read with
  any held key, which is how version control crosses blocks: snapshot entries,
  history pulled from an upstream, merge sides and reused parts
  (a_fork_pulls_and_pushes_through_its_upstream in be-host-stack). So a new read
  of a commit or a sealed payload by a hash or key the server named takes the
  block it is for and goes through `load_head` or `open_for`; `load_reached` is
  only for a hash read out of an object already checked.
- **On the server**: tables `keys` (id, block, kind `data`/`scope`/`escrow`,
  epoch) and `wrapped_keys` (key, `wrapped_by`: a key id or `account:{uuid}`,
  sender set by the server to the storing account). `GetKeys` serves rows of
  blocks the account can view and wraps by its account or a key it is served,
  a page at a time in the order of key ids (`ServerConfig::key_page_bytes`, each key with its
  wraps), which `Peer::refresh_keys` reads through;
  `put-keys` is the one op that stores rows (`create-block` and `move` carry
  none: a write puts `put-keys` beside them). It stores rows only for blocks
  the caller may edit, never replaces a sealed row, and refuses a seal to any
  account but the caller's own and its contacts'.
- **On the client**: `Peer::signed_in` loads every row (kept in the refs
  database, so `Peer::cached` unwraps offline) and unwraps what it can
  (`KeyBook::absorb`). A read meeting an unknown key refetches once, then fails
  with `ClientError::Locked`. `Locked` means the keys answered and none opens:
  a refetch, rotation or root lookup that could not reach the server (offline,
  or one that timed out) fails with that error instead (`is_unreachable`), which
  callers wait out until the next connection rather than show the block as
  locked. `Peer::learn` fetches rows for blocks that appear
  later. A key found missing is not asked for again until keys arrive, so a key
  stored later must be told: a `put-keys` that stores or changes a row tells
  `topic.keys` to every other connection, following it, of the viewers of each block it gave
  a key or wrapped one of under another key, of each viewer it sealed a key to,
  and of every administrator (all an escrow wrap tells). An agent bound to a
  block may not open it; a device attaching again reads every row anyway.
  `Peer::keys_changed` forgets what was
  found missing, and a `learn` or read asking as keys arrive asks once more.
  A key that opens on the device by any path counts as keys arriving too, with
  no word from the server: a fetch, a contact verified after sealing it (a link
  not finished yet, opened by `Peer::refresh_contacts`), the organization's
  administrators learned, or an escrow wrap taken (`settle_escrowed`). Each
  opening goes through `KeyBook::on_opened`, which does what `keys_changed` does
  and tells `Event::KeysChanged` on the connection. The worker then reads its locked nodes again with one `learn`, and
  `Replicas::unlock` opens again each replica found locked that loses nothing by
  it (one with unsealed edits reads again on its next read); a read that keys
  arrived during (`Peer::arrivals`) reads again rather than lock. A submission
  refused for a missing key (by `sealing_key` or `keep_open_edits`) is held
  unanswered, its block locked, and submitted again by `unlock`. The session is
  fine, so nothing would reopen the replica to give it again. One whose replica
  reopened meanwhile is dropped, since the reopening gave its edits again: its
  reply answers `Unanswered`, which the reopened replica ignores as from an
  earlier opening. So a grant may reach
  a device before its key: a share's seal, a pending grant sealed later, or a
  seal key work asks of another editor all unlock it.
- **Moving and sharing.** `Peer::set_parent` into another scope rewraps the
  moved DEKs (old epochs too) under the destination KEK, gives each block a
  fresh DEK and rewraps scope roots inside, all in one write, `[move,
  put-keys(rows)]`. The server checks it at the write's end
  (`keys::Whole::Moved`): every key the move took into the new scope opens
  from that scope's latest key, the moved block's own scope key if it is a
  scope whose key does, else its data key and, below it, each viewable block's
  data key or nested scope's key; one missing refuses the whole write
  (`KeyMissing`). A move within a scope or to Recently Deleted changes no
  keys, and the server checks none. `Peer::share` grants
  and makes the block a scope root sealed to the grantee's verified key.
- **Wrapped only under what is above.** A device wraps a key of a block under
  a scope key only of a block the signed parent records place it at or under
  (`Peer::signed_below`, Access logs below): making a block, moving one (the
  destination's scope, and each block listed below the moved one), making a
  block a scope of its own, and rotating (the top's scope or parent, and each
  block listed below, one log each, `Peer::placed_under`); a block whose
  creation waits in the outbox lies where this device's own record placed it,
  and nothing inside a block whose owner-only key it holds counts as under a
  scope above it. Otherwise it fails with
  `ClientError::Forged`, wrapping nothing: the server's tree alone never puts
  a block's keys where someone else reads them. Where the logs cannot be read
  (offline, or a read that does not answer), a creation takes the scope its
  kept summary names, marked `Creation::unchecked`, and goes up only through
  the outbox, which checks it first; one that does not check, or whose root
  this device never pinned, is set aside (`refs::Aside::Misplaced`).

### Access logs

Who may do what is a record a member signs, and devices act only on records
they check back to the block's creator
(decisions/the_server_never_vouches_for_an_identity.md): given `None` or
`KnowExists`, the server can withhold or destroy a block, never read or write
it, alone or with anyone its managers did not choose, apart from that
decision's standing exceptions and the limits below ("Accepted limits"). Each
block has a log of `be_block_keys::Entry`s, each signed with the
signer's account signing key in a domain of its own (`be3.access.v1`) and
naming the hash of the entry before it. The first is the root: the block's
creator and their keys, made by `Peer::creation` (and `import`) and sent with
`create-block`, also through the outbox. A device takes a root's hash only from
somewhere the server cannot write: making the block (`KeyBook::pin_root`,
before its keys are made, which carry it) or a trusted key of the block
(`KeyBook::opened`). Keys of one block that carry different roots leave it
with none (`RootPin::Conflicting`). A root made with a block stays in memory
until a server takes the creation or the outbox keeps it, and only then is its
log kept (refs `Logs`, `MadeRoot`); a creation refused, failed or discarded
unsent lets the root and log go (`KeyBook::forget_made`), and lifts a conflict
it caused with the one root other keys of the block carried meanwhile. The keys
it minted stay, so what was set aside still reads, and they still carry that
root: the device keeps them as unmade (refs `Unmade`, read before the keys at
start, never collected), pins no root through them and seals, wraps and offers
nothing with them (`KeyBook::unmade`). `Unmade` names keys, not a root, because
a root is fixed by the account and the block (`Entry::root` signs only those):
every device of an account makes the same one, and the same root carried by
another key (one a device of this account sealed once it really made the block)
pins as any does.
A creation the server answers `BlockAlreadyExists`, online, from the outbox or
imported, stands when the served log starts at the root made with it (another
device of the account made it first); a log this account may not read is
someone else's. One made by someone else (a block opened offline with no
summary of it) is set aside as refused. A device that
holds anything of a block's log (a root, or how far it checked the log) never
makes a root for it: making it again is refused as `BlockAlreadyExists` before
anything is pinned (`KeyBook::holds_log`), as a second root would conflict with
the real one and overwrite the kept log its contacts' seals are judged by after
a restart. `Peer::ensure_type` asks the server for a block it has no summary of
before making it. `Peer::access_log` refuses a log whose root is not the
pinned one, or a block whose root conflicts
(`ClientError::Forged`). A key the server seals pins nothing. An escrow wrap,
and a key sealed by a member who is not a contact, open only from a sender the
organization's log names, with the keys it names, and are taken only once the
block's log names that sender with Edit or above (`Peer::settle_escrowed`); a
root taken that way from a member, for a block the device pinned nothing else
for, is weak (`RootPin::Weak`): shown unverified, never passed on, its key
untrusted. A device hands a key on with a root only when it holds that root
pinned in full (`KeyBook::carried_plain`).

Records after the root: grants (`Record::Grant`, a revocation is a grant to
`None`), openings of a kind of settings to anyone inside (`Record::Opening`,
closing is an opening to `None`), parent records (`Record::Parent`) and owner
records (`Record::Owner`) in a block's log, the first three naming where their
signer's right comes from (`Authority`); administrators, members and
invitations (`Record::Administrator`, `Record::Member`, `Record::Invitation`) in
the organization's. `append-log` carries a grant or an owner record (`grant`),
a role or a member record, and `move`, `create-agent`, `delete-agent`,
`remove-contact`, `create-link`, `accept-link` and `delete-link` carry theirs;
`create-block` carries the creator's parent record after the root; the server
appends each once it checks at the log's head as devices check it.

**Members** sign in as they accept an invitation, with no administrator
online. Making one (`contacts::invite`), an administrator signs an invitation
record naming a key the link's code derives (`InviteCode::invitation_key`),
which `create-link` appends. Accepting (`contacts::accept_link`), the accepter
signs a member record naming itself and its keys with that key, endorsed by
its own signing key (`Entry::accepted`, `Invited`; the entry's signer is the
accepter, so a holder of the code names no keys it does not hold), after the head of
the log the link names (`link.head`; read and signed again a few times if the
log moved), which `accept-link` appends. The server never holds the code, so it
can sign no one in, and `check_log` takes such a record only once, by an
invitation still open whose inviter still administers the organization, for an
account the log does not name (`Checked::invitations`). An administrator ends
one with another invitation record (`open: false`, which `delete-link` carries
when revoking); a demotion or sign-out of its inviter ends every one they
opened, for good, even if they administer again. Expiry is the server's alone: no device can check its clock. An agent is
signed in by its maker as `create-agent` makes it. Every removal of a person is
an `append-log(member)` signing them out, by an administrator, with the signed
revocations of their grants. A sign-out names no keys (`Member::identity` is
none), so one of a person no record signed in (the server added them) takes
nothing from the server's word; an agent's is `delete-agent`, signed out where the
log names it (the personal organization has no log). A proof path keeps every
invitation record whole, so a member's own record checks on theirs.

**Owner records** move a block's ownership:
only its owner (the root's creator, or whom the last owner record named, held
as `Checked::owner`) signs one, naming another account and the keys it verified
for them, sent as an `append-log(grant)` with Manage (`Peer::transfer`, the share
dialog's crown). The new owner holds Manage and no grant may name them; the
former keeps Edit (`FORMER_OWNER`), so every Manage they held by ownership,
directly or as authority for a record below (`Authority::Inherited`), ends
there, and the record lowers someone, outdating the epochs before it. A proof
path keeps every owner record whole, and a reduced entry says whether it is one
(`Reduced::owns`): reduced, it still leaves its signer, the owner then, Edit,
and the owner unknown. The server moves the block's author with it (giving the
new owner a Manage row, since authorship alone gives Manage only while they can
view the parent) and refuses it for blocks that belong to what holds them
(owner-only blocks, settings, edit kinds, search indexes, an Unfiled). A
transfer has no accept step: the new owner holds the block as soon as the
record is appended, and can leave it only by giving ownership on, since no
grant names or lowers an owner; holding Manage, they may also trash it. The
server takes a transfer only to a member who is a contact of the giver, and
refuses one to an account already at `MAX_BLOCKS_PER_ACCOUNT`, checked after
the contact check so the giver learns an account's count only of a contact. The former owner, the creator included, is then
a member like any other, whom the new owner may revoke.

**Parent records** place the block (`Placement`:
the top, Recently Deleted, or under a block with the hash of that block's log
root, which the signer pinned in full: `Peer::placement`), signed by a member
with Edit or above on it (from its log, administering the organization, or a
block above), and never lower anyone. A parent whose root this device has not
pinned has its keys asked for first; with none that pins it, a block made under
it fails as `Locked`, and with no connection to ask over, as unreachable. A log's last places the block now
(`Checked::placement`). A record on `Authority::Inherited` names the chain of
blocks up to the one it rests on, each with a head of its log (`Link`), and
checks only if each is where the one before lay as of its head, starting from
the block's placement at the record's place, each log checked from the root
the placement under it names (`be_block_keys::answer`), at most
`MAX_ANCESTRY` long; the server takes it only with every head current. So the
server can neither place a block under an ancestor someone else manages, nor
serve a made-up log for a real ancestor, nor count a block above that it lay
under only at another time, and a grant or opening resting on a manager above
seals like any other. `Logs` checks each log once as of each head, so
authority resting on authority costs linear in the records. A device reads the
logs a log's chains name (`Peer::logs_around`) only when a record in it names
authority outside it, and builds a chain up its current placements
(`Peer::held_above`), each log checked as `checked_access` checks it, so one
served short of what it kept counts as withheld. An agent's account id is derived from its maker and its keys
(`agent_account`). Revocations the server makes on its own wait in key work
(`unsigned-revocations`) for a manager's device to sign them. Members no
record signed in are listed in `unsigned-members`: an agent its maker's device
signs in at its next `tend`, or a person the server added on its own, which
the members dialog warns administrators of, for them to remove.

A company organization has a log too, rooted at its creator:
`contacts::create_organization` picks the organization id, signs the root and
sends it with `create-organization`, registering the account's keys first if
the server holds none. What an account pins for an organization is
`contacts::Pinned`, the root's hash and the escrow key, kept in the account's
block (Contacts below; `contacts::keep_pin`, `contacts::pinned`) and in the
session: the creator pins it on making it, a member from the invitation, which
carries it sealed with the code, and an agent from its maker. Each arrives
sealed by the account for itself (`seal_pin`) as a `letter.pinned` left by the
write it came with (`create-organization`, the invitation's `accept-link`,
`create-agent`), which a device takes into the block; an agent, which has no
block, keeps the letter. A pin carries when it was pinned (`Pinned::made`,
in milliseconds, stamped as it is taken or saved above any pin the account
held for the organization), and a device takes one only over an older,
whatever its place in the mailbox, so an old pin the server sends again is
never taken. A device working in a company organization reads its
pin from there as its session opens (`Peer::open_session`), so a new device
needs only the identity secret to open the organization's escrow chain.
`Peer::organization_log` refuses a log whose root is not the pinned one. The
personal organization has no log.

A device takes a log only checked from its pinned root to its head
(`be_block_keys::check_log`): each entry names the hash of the one before and
is signed by its signer's current key. Every root names its kind
(`LogKind`), so a block's log never checks as an organization's. A checked
log (`Checked`) also tells whether it reaches an entry (`holds`) and each
account's level as of any entry it reaches (`level_as_of`; for an
organization's log, administering it is Manage). `check_log` is
`Checked::extend` from the empty state (`Checked::unchecked`), and a device
checks each signature once: a whole log it checked, a block's or the
organization's (kept under the organization's id), is kept with the refs
(`KeptLog::checked`), and `checked_access` and `checked_organization` check
the log served next on from it (`Checked::resume`). That takes the served
prefix only when every entry names the one before and the last kept hashes to
the kept head, verifying no signature there, and refuses the log as forged
otherwise, as it does a log at least as long that does not reach the kept
head (one shorter is checked whole, and withheld). Within a run, a device
reads a whole log again by naming its last entry (`Peer::served_access`): the
server sends only the entries after it, and the first must name it as the one
before. A proof path is read and checked whole each time
(plans/access_logs_checked_once.md has what is left: proof paths, authority
from another log, keeping the entries across restarts, and the server).

**Who is served what.** A sealer must see every entry up to the head, since
an entry reduced to its hash could be the revocation of whom it seals to, or of
their granter; so whoever holds Edit, who seals (and reads all the content
anyway), is served whole logs: the block's, every ancestor's it may view, and
the organization's. That reveals little: everyone granted on an ancestor
already reaches the block (decisions/block_access_is_the_highest_grant_on_the_path_up.md)
and its managers see them (`list-access`); an editor newly learns that set,
the ancestors' grant history and their parent records, and of the
organization's log, who is or was an administrator. Since a block's creator
holds Manage on it, every member who has made a block is served the
organization's whole log, and so its member list; whether to narrow that is
open. Readers below Edit never seal and are served proof paths
(decisions/editors_are_served_whole_access_logs.md). No log goes to
`KnowExists`, which anyone gets by naming an id in their own block's
references: the log names every grantee. A `Join` member is served a proof
path of its own right and of the session's lease holder's ("Hosted channels").

**Proof paths.** A log is
served whole to a member with Edit or above (on the block or a block below it;
for the organization's, on any block, or administering it), who seals, and to
anyone else as a proof path (`be_block_keys::proof_path`): `Step`s, each an
entry whole or reduced (`Reduced`: the hash of the entry before it, whether it
lowered anyone, and the digest of the rest), which hashes as the whole entry
does. Whole on a path: the root; every entry naming the reader or an account
that sealed it a key of the block (of any block, in the organization's log),
and so on for each whole entry's signer, so each checks at its place; what the
reader signed (its agents); every opening and parent record (a parent record
reduced still says it is one, `Reduced::places`, and leaves the block's
placement unknown to the reader, so a server hides no move); and in the
organization's log every administrator record and every record about anyone
who ever administered it. A block above one the reader views, which it may not
view itself, is served as a path proving too whoever signs below on authority
from it, and whoever sealed the reader a key, so a grant from a manager above,
or a seal from an editor above, checks for its reader.
So a reader learns no other member's grant. Every entry says whether it
lowered anyone (`Entry::lowers`, covered by its hash outside the digest),
which `check_log` refuses unless it is so and devices set by signing through
`Checked::next`. `check_log` reads a path as a whole log (`Logged`) and marks
it `partial`. How each check reads one:

- `holds`, `current_at` and `outdates` are exact: positions and
  `last_lowering` are the whole log's, so a revocation the reader is shown only
  reduced still outdates the epochs before it. Kept state, withheld and stale
  answers work as for a whole log, a path having the whole log's hashes.
- Nothing is sealed by a partial log to a grantee or an administrator
  (`sealing_target`), nor an escrow epoch made by one: a reduced entry may be
  the grantee's revocation or the administrator's demotion. A viewer still
  offers a kind its log opens to its contacts inside: openings are whole, and
  any reduced lowering after the last counts as closing it (`open_inside`).
- `Checked::next` signs on a partial log only a grant to the signer (a reader
  leaving) and an agent's member record, which the path proves `lowers` for;
  any other record is refused.
- No key is wrapped under an escrow epoch while the organization's log hides a
  lowering (`Checked::hides_lowering`, `KeyBook::escrow`): an honest path of it
  reduces no entry that lowered anyone.
- An organization path that reduces an entry that lowered anyone hides a
  demotion (an honest one reduces none): `check_log` leaves it naming no
  administrator and no level, so no seal, escrow wrap or epoch is taken by it.
- A seal's sender is judged by the records the path shows of them
  (`sender_may`, `settle_escrowed`, kept `levels`), but the server picks whom
  a path proves: a sender it leaves out is never named in the block's log,
  and is taken only if `Peer::held_above` finds them holding Edit on a block
  above, by the paths of those blocks the server serves, which for a reader
  below Edit it picks too. So for a reader below Edit the sender check is in
  effect a verified contact check. Accepted ("Accepted limits"): a reader below Edit seals, wraps and
  writes content under nothing, though it relays its presence under such a
  key.
- What a path still reveals, accepted: the log's length and where each
  lowering sits; the whole level history of each sealer and of the chain of
  signers behind them; who signed each opening; of the organization's log, the
  whole administrator roster and its history; and of an ancestor's path (served
  to a reader below it, `reads_below`), its creator and root, every placement
  it ever had and who made each, and the level history there of every mover
  and every signer below on authority from it. Fixing the length would take a
  commitment to the log's state instead of a chain
  (decisions/editors_are_served_whole_access_logs.md).
- A path is kept (`KeptLog`) only up to its last whole entry, so a tail the
  server made up after the real head never leaves later logs withheld; the
  newest lowering a path ever showed is kept past that, so a later path that
  drops the tail outdates the same epochs.

**Epochs name their heads**, so a server that withholds a revocation from a
device cannot have that device seal the next epoch to the revoked member.
A key minted here
names the heads of its scope's log (a scope key's own block; for a data key,
the scope it is wrapped under) and the organization's, as this device last
checked them (`KeyBook::note_log`, `set_organization`; a scope whose log it
never checked names its pinned root, so a device whose logs are stale names
stale heads), and a rotation reads the logs of the scopes it rekeys first.
The heads ride inside the key (`CarriedKey::heads`), so they come from its
maker through every sender (`KeyBook::heads`), and no one who never held the
key can name other heads for it
(decisions/key_epochs_name_their_heads_inside_the_key.md). An escrow epoch
names the organization's head in its signed record (`EscrowKey::head`).

- **Sealing.** Key work seals a key to a grantee or a contact inside only once
  the block's log, as served now, reaches the key's block head, and to an
  administrator only once the organization's log reaches its organization
  head (an escrow epoch's, its signed `EscrowKey::head`); each then counts as
  of its head now (`Peer::sealing_target`). So a server that withholds a
  revocation, a kind's closing or a demotion made before an epoch has that
  epoch sealed to no one by that log until it serves the log that far.
  Nothing tells the person meanwhile: the grantee sees the block as not
  shared yet.
- **Outdated epochs.** A log's `last_lowering` is its newest entry that
  lowered anyone's access (a revocation, a downgrade, a closing; a demotion or
  an administrator signed out). An epoch naming a head before it is outdated
  (`Checked::outdates`): its subject may hold it. So is a key naming no head,
  or one the log does not reach even read again, once the log has lowered
  anyone (it counts as named at the root), and a key opened through an
  outdated epoch, whatever it names (`KeyBook::outdated`): otherwise a revoked
  member could wrap a made-up key under an epoch they hold, which no seal names
  them in, and have it listed as newest. Key work seals an outdated epoch to no
  grantee, and treats the scope as due a rotation when it is the scope's
  newest (`Target::Outdated`); to an administrator the log names now it seals
  any key, so one made after a demotion still gets the settings' keys. The
  organization's log outdates the same way every key under the escrow (any
  but one an owner-only scope covers) naming an organization head before its
  last demotion (`KeyBook::demoted_since`):
  the demoted administrator may hold it through escrow, so a demotion makes
  every scope due a rotation: at its next write, and in each device's `tend`
  for every such scope it may edit (`Peer::rotate_demoted`), since a grantee
  inside a scope cannot rotate it (its new epoch is wrapped under the scope
  above). A
  device about to write or wrap under a data or scope key its logs outdate
  rotates the scope first (`Peer::sealing_key`, `scope_key`), from the highest
  scope above it whose newest key is outdated too (`Peer::rotation_top`), so
  one rotation covers a tree a demotion outdated whole, judging by the
  logs of the scope and of every scope above it the key was opened through,
  and the organization's (`KeyBook::logs_to_read`, `Roots::through`, at most `keys::MAX_PATH` deep,
  longer counting as outdated), as it last checked them (key work, sealing, a
  contact's seal or a rotation reads them; one it never read since starting,
  it reads first, each once per key in a run). How far it checked each scope's
  log is kept with the refs (`refs::KeptLog`, `KeyBook::note_log`;
  `refresh_keys` reads each scope's log once as its keys arrive, and a block
  made here is kept at its root, eight scopes read at a time): a log served
  later that does not reach the kept head is withheld, and `checked_access`
  refuses it (`ClientError::Withheld`), so nothing seals, settles an escrowed
  key or judges a contact by it. The organization's log is kept the same way,
  under the organization's id: `checked_organization` refuses one short of it
  and leaves the organization as held, and with none read this run an escrow
  epoch is current only if it names a kept entry from the last demotion on,
  key work seals to no administrator, and a contact counts as administering
  only as kept. With it, a failed read or offline, an epoch is current only
  if it names one of the kept entries from the last lowering through the head
  (`KeptLog::current`), and a contact's seal is judged by the levels kept at
  that head (`KeptLog::levels`). A scope neither kept nor readable makes a key opened through
  another outdated. A read before a write is given up after `LOG_READ_LIMIT`,
  leaving what was kept, and none is made for a scope whose creation still
  waits in the outbox. A rotation the server refuses fails the save as
  `Locked`, and so does any write that goes up at once (a part, a relay, a
  journal entry, metadata: `Peer::sealing_key`); one that cannot reach the
  server fails it as unreachable. Offline, a person's edit
  for the outbox is never refused (`sealing_key_kept`, for a commit and the
  content it holds): it is sealed under the epoch at hand and kept, and judged
  again before it is sent (`Peer::send_pending`, `KeyBook::shown_outdated`).
  When the logs show its key outdated, the drain seals every commit of the
  block waiting in the outbox again (`Peer::reseal`, `Drained::Resealed`):
  content, commit, metadata and journal state, under the key `sealing_key`
  gives now (a new epoch, which it makes first), each on the commits it went
  on as sealed again, in its place in the outbox, and sends it. A write on a
  commit sealed again goes on the one in its place (`Peer::resealed_as` names
  what a commit replaced, for a replica that kept them). Only while this
  account may still write the block as its signed logs show now (Edit or
  above in its log, administering, or from a block above), and only under an
  epoch the logs do not outdate; otherwise, or when no epoch can be made, the
  edit is set aside among the unsent (`refs::Aside::KeyOutdated`). Whatever
  path uploads (an outbox commit, a snapshot or a fork through `Peer::hold`),
  an object the server lacks that is sealed under a key the logs show outdated
  is not uploaded (`ClientError::Outdated`), so what was first sealed never
  leaves the device. `write_content`, public, is strict; only the outbox's own
  writes keep. As for an edit the server refuses, what is made of the block
  after one set aside waits with it until it is discarded. An object whose block's scope cannot be told
  counts as outdated. A log answer older
  than one this run already took is stale, not withheld (`keys::Served`). A
  local account judges nothing (there is no server). An escrow epoch the organization's log outdates
  is wrapped under no more, so keys wait unescrowed for the next, with no
  warning (the gap after a demotion is no fault). So an epoch a server got a
  withheld device to make, naming a head before a revocation, is not written
  under by a device that holds the revocation.
- **Taking a contact's seal.** An escrow key only from a contact the
  organization's log names an administrator at its head now. Any other key
  only from a contact the block's log gives Edit or above, or the
  organization's log has administer it, as of the head the key names, or as
  of the log's head here when this device's log does not reach that head or
  the key names none (with no log read this run, as of the kept head); a data
  key only from an administrator, as only the settings' are sealed to anyone; one the block's log names but not by then held nothing
  (`KeyBook::sender_may`). From a contact the block's log never names, a key
  only once this device found, as that key arrived, the contact holding Edit
  or above on a block the signed parent records place it under
  (`Peer::held_above`, `KeyBook::vouch`): that wrap alone (key, sender and
  digest of what was sealed) is kept as vouched for (`KeptLog::vouched`), so
  another wrap of the key, or a contact revoked above since, hands over
  nothing new. A key of a block whose log this device never checked is
  taken; `refresh_keys` takes this account's own keys first, so the roots they
  carry are pinned, then reads the log of each block whose root it pinned
  before taking a contact's seal of it.
- `tend` signs key work's revocations before the rotations they call for, so
  the new epochs name heads after them.

### Rotation

When an account stops being able to view a block (revoked, left, moved away,
contact or member removed), the server records the block in `rotations`, drops
that account's wraps and ends its sessions there. `query.key-work` tells a
client what it can do, and following it tells `topic.key-work` as it may have
changed: rotations on blocks it may edit, scope keys not yet sealed to
someone who should have them (`unsealed`, whose list is `keys::sealed_to`), and
scope keys not yet escrowed. `Peer::tend` does all of it; the worker calls it
on connect, `MailboxChanged`, each `KeyWorkChanged` and after its own changes.

`Peer::rotate` sends one `put-keys(rotate)` for a subtree: new epochs of every scope key,
every data key rewrapped, a new data key per block. The server takes it only
whole. Each block it lists must be placed where the server lists it by the
signed parent records (`Peer::signed_below`), or nothing is sent. Content is not resealed, except that the lease holder of a live session
on a rekeyed block seals once more (`Session::rekey`), which reseals the parts it
reuses under the new key (`Vault::write`). A rotation nobody can do yet
waits until a client that may edit the block tends. A kind its own log opens
to anyone inside (`Checked::open_inside`, guides/settings.md) is also offered
by any viewer to its own contacts the server lists among the viewers, since the
owner's editors may know none of them; the server's `opens_inside` alone
offers nothing.

A demotion (or an administrator signed out) rotates nothing at once: it
outdates every epoch under the escrow ("Outdated epochs"), and two paths
rotate them. A write, block made or relay in an outdated scope rotates it
first (`Peer::rotate_outdated`); and the server tells every member's devices
that key work changed on a demotion or a member's removal, whose `tend`
rotates every outdated tree this device may edit at its top
(`Peer::rotate_demoted`, judged from the signed logs as a reseal judges it,
`Peer::may_write_signed`). Both start from the highest outdated scope above
(`Peer::rotation_top`), one `Rotate` per tree, as a revocation on its top
costs: rotating each scope from below would rotate the scopes above again at
each level, since a scope's new epoch is wrapped under a new one of the scope
above. So within one round of key work every tree some connected editor at
its top can rotate is rotated, the organization's content keys once over. An
administrator's device holds most scopes through escrow and tries every
outdated tree in one `tend`; administrators tending at once race, and each
reads a tree's keys again just before rotating it (`refresh_keys`) and skips
one already rotated, so the later ones mostly skip, though two at the same
moment can both rotate it. A member who reaches a scope only from inside it
cannot rotate it: until an editor above tends, their saves there are refused
as `Locked` (the host submits them again as keys arrive), and an edit made
offline is set aside (`Aside::KeyOutdated`) if no rotation reached the scope
by the time it is sent. A tree whose rotation fails stays listed in
`Peer::pending_rotations` (its top, since when, how many times, the last
error) and logged at each failure, until one goes through or it is current
again, so a grantee stuck on `Locked` can be diagnosed from the editors above.

Not built yet: a move into another scope lists the subtree a level at a time,
and old data keys of a block this device cannot open are not rewrapped, so
that history stays closed to the new scope.

### Grants the server changes first

The server may withhold at once, but never adds access a device did not sign.
Where it acts first, a device signs after:

- **A contact link's grant.** Its maker does not know who will accept, so no
  grant can name them: accepting applies the server row at once, the scope key
  reaches the accepter sealed with the link's code, and the maker's device
  signs the grant when it finishes the link (`finish_links`). Until then no
  other device seals the accepter a later epoch.
- **Removals the server makes on its own** (agents collected or deleted,
  `remove-contact` in another organization) revoke the server rows at once and
  wait in key work (`unsigned-revocations`) for a manager's device to sign
  them; a device sealing by the log also checks the server's row, so nobody
  seals them a new epoch meanwhile. Removals made from a device carry their
  signed revocations.
- **Who views inside.** A kind its log opens to anyone inside is offered by a
  viewer to its contacts the server lists as viewing a block inside: that
  they view stays the server's word, but only for a kind a manager opened.
- **The server's own account** holds the organization settings' scope key by a
  row with no record (decisions/server_enforced_settings_are_shared_with_the_server.md);
  no record can name the server, so it is never a sealing target by grant.
  The server opens such a wrap by its sender's current key, so a new phrase has
  it seal those wraps again from itself (`ServerKey::reseal_from`), and an
  administrator's device sharing the settings again replaces its seal.

### Share links

"Anyone with the link" on a block (`be_block_client::share_links`,
`be-server/src/share_links.rs`). A link is the out-of-band start every sharing relationship
needs (decisions/share_links_are_an_out_of_band_start.md): it grants one block to whoever
opens it and makes nobody a contact, so it is no `link-kind` of the contact link family.

- **Making one** (`Peer::make_share_link`, by a manager): an `InviteCode` whose lookup id is
  the link's id (`{server}/#share/{CODE}`, `invite_link::share_link`), a key pair it derives
  for the block (`InviteCode::share_key`), and what the link hands its opener
  (`share_links::Handed`: the block's scope key as it travels, with its log's root, and the
  organization log's root), sealed with the code (`InviteCode::seal_share`). The block becomes
  a scope of its own, as for `Peer::offer`. A share link record (`Record::ShareLink`: the
  link's key, its level, View or Edit, whom it admits, `Admits`, and its signer's authority)
  opens it in the block's log, signed with Manage. The maker's account block keeps the code
  (`AccountBlock::shares`) until the link is revoked, so the share dialog shows the link again.
- **Opening one** (`share_links::open_share_link`, the account's own call): the opener reads
  the link (`query.share-link`), opens what it hands with the code, seals the scope key to
  itself, and signs its own grant with the link's key (`Record::Opened`, `Entry::opened`),
  endorsed by its own signing key over the grant and the entry it follows, after the head of
  the block's log; the server appends it and applies the grant. `check_log` takes such a
  grant only for its signer, at most the link's level, raising them, while the link is open,
  and for an account the log names already only with the signing key it names; the server
  never holds the code, so it grants no one. `Checked::linked` names whom a link granted, and
  the access list marks them. be-host opens one pasted into Open a share link, in the host
  menu, or from the web build's address bar (`share_link.rs`): it reads the link first
  (`preview_share_link`) and asks, naming the block (its name as the maker saw it, sealed
  with the code), the organization, what it grants and whether the account joins as a guest,
  and opens that organization when it is not the one open.
- **Join links** (plans/hosted_blocks.md): a link of level Join carries no key, and its opener
  reads nothing stored. Having no key that carries the block's root, it checks a host by the
  log as served, taken when it holds this account's own `Opened` grant (`Peer::joined_log`):
  its endorsement covers the entry before it, so the chain to the root is the account's own
  word. A hosted block (an app's manifest runs a host component on its type,
  `discovery::hosted`) offers Join in the share dialog's link row, first and by default; any
  other block never does.
- **Keys after a rotation.** The scope key a link carries opens the epochs current when it
  was made. Later epochs reach an opener through key work's `unsealed`, as any grantee: their
  grant names their keys, so an editor's `tend` seals to them by the block's log. Their
  device takes that seal by the keys the block's log names for its sender, when no contact or
  organization log names them (`KeyBook::log_names_editor`, then `Peer::settle_escrowed`
  checks the sender's Edit in the log); `refresh_keys` reads such a block's log first. An
  opener after a rotation so waits for an editor to come online.
- **Revoking one** (`Peer::revoke_share_link`): a share link record closing it, by a manager.
  Every grant made through it falls in the log (`check_log`), even where a manager had given
  the opener a lower grant before (the link's raised it, and revoking leaves none), the server
  revokes their rows, and the block rotates as for any lost access. The share dialog asks to
  confirm and says so. On a proof path the grants a link made may be
  reduced, so whether the closing lowered anyone is the record's signed `lowers`.
- **Guests.** A link admits members of the block's organization (in the personal organization,
  every account) or, where the policy allows it, any account on the server. One from outside
  the organization joins it as a guest (`OrganizationRole::Guest`): a member row holding only
  grants made through links, reads none of the organization's settings, is served its log as
  a proof path, so it learns no member (but a guest with Edit on a block inside a workspace is
  served the whole log of each block above it, as any editor is, since key work seals by
  them; that is meant), and may not make blocks, agents or links, move or grant
  (`client::guest_may_write`), is no member for a grant or a pending grant
  (`store::is_member_of`), and leaves when its last grant falls
  (`share_links::drop_idle_guest`). Accepting an invitation makes it a member.
  A guest pins the organization log's root the link hands it, when it pinned none, so it checks
  records an administrator signed.

### Escrow (company organizations)

The escrow key (`be_block_keys::EscrowSecret`) is an X25519 key: a key row of
kind `escrow` on the organization id, sealed to each administrator. A device holds its secret apart
from the content keyring (`KeyBook`'s escrow secrets), since it is never an
AEAD key: an object whose header named the escrow's id opens with nothing. The
first administrator device
to open the organization makes it (`CreateEscrow`), and `mint_scope` seals
every new scope key to it too, so administrators open everything. Demoting or
removing an administrator records an escrow rotation, and tells every
member's devices that key work changed, whose `tend` reads the organization's
log and rotates every scope under the escrow it may edit (Outdated epochs
above): the next epoch is signed
over `EscrowKey::chained` by its maker's identity signing key, in the escrow
chain's own domain (`RecoverySecret::sign_escrow`, `SigningPublic::verifies_escrow`), an administrator the
device's checked organization log names at its current head, and covers the
organization log's head it was made under (`EscrowKey::head`): the server
takes it only at its head now, and a device takes an epoch only once its
checked organization log reaches that head (`verify_escrow`). Only an epoch
this account made itself is taken for holding its secret; any other only
through the signed chain. `make_escrow`
seals it to the other administrators the log names who are contacts. A minted escrow key becomes the
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
reseals it with everything else the account sealed for itself. The account's
own block (Contacts below) is one too.

A member wraps only under an escrow key it pinned from somewhere the server
cannot forge: its own, the invitation (`InviteCode::seal_escrow`), an agent's
maker, or a secret an administrator contact sealed to it. It follows the chain
one signature at a time (`keys::verify_escrow`) and pins the newest verified
in the account's block (`Peer::save_pin`); an unverifiable newer key is not used, and the host's menu shows
`KeyBook::escrow_warning`.

### Identity, new phrases and devices

- **Identity**: a twelve-word recovery phrase derives an X25519 key pair, and
  from its secret an Ed25519 signing key (next section); the server stores both
  public halves (`SetRecoveryKey`). A device keeps the secret
  (`HostStateStore::identity`); without it, or with one that does not match,
  the account opens on the unlock screen (`be-host-stack/src/keys.rs`,
  `ui/keys.rs`). A local account (`ServerLocation::Local`) is never asked to
  save its phrase: its secret sits beside its database anyway, so be-host saves
  the first phrase without the check screen, as the dev workspace does. A
  device that holds an account's secret, told by a remote server that the
  account has no key, makes no new phrase, which would replace the secret it
  keeps, and says so (`KeyState::loaded`, `SERVER_LOST_KEY`).
- **A new phrase** (`contacts::set_recovery_key`) must reseal every row the
  account sealed for itself, which the server checks, so only a device with the
  old secret can make one; it reseals too every row a contact sealed to it that its
  records in the account's block reach, which the server cannot check. The
  account's block's keys are among those rows, so the new phrase reads its
  history; and in the same write the block starts a new key epoch whose scope
  key is sealed to the new phrase alone (`AccountRecords::epoch_for`, a
  `put-keys` rotation beside `set-recovery-key`, in the personal
  organization), so the old phrase reads nothing written to it after, even on a
  device that kept every key it opened. A device whose block does not open
  (a newer phrase was taken elsewhere) takes no new phrase. It
  also appends a retirement (`be_block_keys::Retirement`) to every access log
  the account signed in (`recovery-key`'s `logs`): endorsed by the old signing
  key, signed as an entry by the new one, and checked by the server and every
  device, which take nothing the old key signs in that log after it. The
  endorsement is bound to no time or epoch: whoever holds an old phrase can
  retire it in any log where it is still current, at any later moment, so a
  thief who retires first wins those logs ("Accepted limits"). Each
  verified contact gets a key change naming both new public keys
  (`contacts::new_identity`, `RecoverySecret::seal_key_change`), left in the
  contact's mailbox (`letter.key-change`) by the same write; a contact's device
  follows changes one at a time from the key it holds, reseals what that
  account sealed to it (`ResealKeys`), writes its record in the account's block
  and deletes the changes it followed. Before sending, the device follows its own
  contacts' waiting changes, checked against its pins, so the changes seal to
  each contact's newest key; that step is best-effort, so a contact's mailbox
  never holds up a new phrase (`accounts::set_recovery_key`). The server, in
  the same write, seals again from its own account each key the account sealed
  to it (the settings' keys, `ServerKey::reseal_from`): it opens such a wrap by
  its sender's current key, so it would otherwise stop reading the policies
  and refuse what they govern (`PolicyUnreadable`).
- **Inviting**: the only way into an organization, and it hands over no key.
  An invitation is a link of kind `invitation` (see Contacts below): the
  inviting device makes an `InviteCode` (200 bits) and names the link by
  `InviteCode::lookup`, so it is `{server}/#invite/{CODE}` (`invite_link.rs`).
  It carries the inviter's card and what the inviter pinned for the
  organization (`Pinned`: its access log root and escrow key) sealed with the
  code; accepting it returns the invitee's card as any link does, pins both
  and joins the organization as an editor, signing the invitee into its log
  with the key the code derives ("Access logs", Members). It names no email: the link is a bearer
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
  contact), whatever its age. Its
  inviter's account's block keeps its code (`contacts::kept_links`), so they
  copy the link again; any administrator
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
  be mistaken for each other. A block's signed log is signed the same way: the host signs what a type module
  appends as the module's app (guides/type_modules.md, "Signed logs"), and only that
  app's editors may append. What it signs commits to a random salt sealed with the
  payload (`Said::hash`), so neither the server nor anyone else without the block's
  key can confirm a guessed entry against its signature, before or after a redaction. `signing-block` names the block signatures bind to, which a record that names its chat or block inside its bytes uses (the block the revision is of, in a revision preview, where `editor.block_id()` is the preview's own). Signatures verify only in the app that made them: another app's `verify` answers `Bad`, which is not a forgery verdict.
- **Apps** never hold it: host-services' `sign` and `verify`
  (guides/apps/host_services.md) answer from the worker (`be-host-stack`
  `authorship.rs`), in the block the editor shows.
- **Verifying** (`Peer::verify_authored`, answering `Signer`): the refs
  database keeps the signing keys known per account (`RefsDb` table `Signers`,
  never forgotten, so what was signed before a new phrase still verifies). A
  contact's key, from its card or a key change, and the account's own are
  verified, the newest being current: an older verified key answers `Changed`,
  since someone holding the old phrase can sign with it. A newer verified key
  marks the older ones replaced as of the server's clock as the device estimates
  it (`KnownSigner::replaced`, `Peer::server_now`); a record that keeps when the
  server took it (`verify`'s `arrived`, chat's `Message::arrival`, every entry
  of a signed log, `Peer::verify_entry`) and arrived
  more than a minute after that answers `Replaced`: louder than `Changed`, but
  not `Bad`, since the account's own devices not yet told of the new phrase and
  sends held offline sign with the old key too, as does someone holding the old
  phrase. A record with no stamp, or arriving between the new phrase and this
  device learning of it, stays `Changed`. An account with one is checked only against verified keys, after
  rereading the contacts once, so the server cannot sign as a contact. Any other
  member's key comes from the server (`query.signing-keys`: the caller, its
  contacts and the members of the organization it names, which in the personal
  organization is every account) and is pinned the first time, unverified; a
  different key the server names later answers `Changed` for what it signed.
  A contact whose chain of key changes broke ("Contacts") has every verified key
  marked superseded (`RefsDb::supersede_signers`), its newest too, so what they
  sign answers `Changed` or `Replaced` until a link verifies a key again;
  the key a link verifies is current again with no replacement time, so a later
  break is timed by itself.
  Nothing known that made the signature answers `Bad`; a record found `Bad`
  after asking the server is remembered for that connection (`Forged`), so a
  forged row shown again asks nothing until the device reconnects.

### Hosted channels

A hosted block's members speak to its host over a private channel
(plans/hosted_blocks.md, "The channel"; `be_block_keys::noise`,
`be_block_client::peer::hosted`), carried by the session's `host` send
(guides/blocks/server.md, "Live sessions"), which the server routes and
cannot read.

- **The host is an account.** The server names the lease holder's account; a
  member checks the signed logs give it Edit or above (`held_with_key`: its
  record in the block's log, else the nearest block above by the signed parent
  records, as `held_above` finds it, else administering the organization) and
  takes the identity key the record giving it names (`Peer::join_host`), else
  refuses it as forged. A member that pinned the block's root checks the log
  as `checked_access` does. A `Join` member holds no key of the block, so
  nothing carried it the root: it takes the root of the log as served once
  that log holds a grant to it signed with a key it verified for the contact
  who signed it (`joined_log`), and pins it weakly, in memory
  (`KeyBook::pin_vouched`, `RootPin::Weak`), so a log rooted elsewhere later in
  the run is refused ("Accepted limits"). The logs of the blocks above, which
  the walk up reads, it is served as proof paths through `reads_below` (its
  own block lies below them), proving the lease holder there too.
- **The handshake** is Noise IK (`Noise_IK_25519_XChaChaPoly_SHA256`: this
  crate's one seal in place of ChaChaPoly; the cipher is a private parameter,
  and a test runs the same code as `Noise_IK_25519_ChaChaPoly_SHA256` against
  a published vector) between the two accounts' X25519 identity keys, the
  member initiating, with the prologue `hosted_prologue(block, head)`: the
  block and the head of the log the member checked the host at, which its
  first message carries in the clear before the Noise message, with its
  account id as the payload. The host (`Peer::admit`), served the whole log,
  answers only if its log reaches that head and the signed logs give that
  account `Join` on the block itself, or View or above from anywhere up its
  path (View holds Join; Join itself never passes down), now, with the key the
  handshake carried; a member revoked since, or one speaking with another
  account's key, is refused. IK's first message can be replayed, so the host
  counts the member as admitted only once the member's first message on the
  channel opens (`Admitting::confirm`); the member sends one at once.
- **Messages** (`noise::Channel`) each carry a 64-bit counter, the seal's
  nonce, and a side takes only the next one: a message out of place (dropped,
  held back, replayed) or changed breaks the channel for good, and the member
  handshakes again, as it does after a host change or a reconnect. A sealed
  message the server refuses (`SlowDown`) is sent again as the same bytes,
  never sealed again (`Channel::seal`). So the
  server can neither read, forge, reorder, replay nor silently drop a
  transport message; the handshake's own first message it can replay, which
  admits no one. Keys and the chaining key are wiped when dropped.

### Accepted limits

What the signed logs still leave to the server, or to the server with someone
who lost access. Cited from code by these names.

- **What a revoked member held** stays readable to them: every epoch before
  the rotation.
- **Exported logs.** An export's entries each verify against their block, app
  and author (`Peer::verify_exported`), but the file carries no head or proof:
  whoever edits it can drop or reorder entries unseen.
- **Withheld revocations.** A device knows a log only as far as the server
  served it, or as it kept it (`KeptLog`). A server that withholds a
  revocation, a closing or a demotion from a device keeps an older epoch
  current there, and the epochs it makes name heads before it; a device that
  holds it rotates them away when it writes or tends. Nothing tells the person
  when an epoch is never sealed because its head is never served: the grantee
  sees the block as not shared yet. A revocation, or a move, a device never
  held in any run it cannot catch: a server can serve a device new to a block
  a log cut short, and a device that never checked past a move wraps under the
  old parent's scope only what it would have before it.
- **Made-up keys.** A key a contact seals is taken as their Edit (or
  administering) as of a head they choose among those the log holds, so a
  member revoked since, with the server, can seal a made-up key naming a head
  before their revocation; a device whose log outdates that head writes under
  no such key. Never an escrow key, taken only from an administrator at the
  organization log's head now.
- **A sender the block's log never names.** Their key is taken only for the
  one wrap `Peer::held_above` vouched for as it arrived (`KeyBook::vouch`,
  kept as key, sender and digest of what was sealed, `KeptLog::vouched`),
  when the signed parent records showed them holding Edit or above on a block
  above as of those logs' heads then; any other wrap of the key, or a later
  key, is judged as it arrives. A server that withholds the sender's
  revocation above from this device gets that wrap vouched for and taken. Such
  a key is written under only while it names a head with nothing lowering
  anyone after it, or, naming none, while the log never lowered anyone.
- **The first key of a block this device never checked** is taken from a
  verified contact before any log of it is read (`refresh_keys` reads the log
  of each block whose root it pinned before taking a contact's later seals of
  it), so a contact, with the server, can hand a device new to a block a key
  of it naming any heads.
- **A reader below Edit checks a sealer only as a contact.** The server picks
  whom a path proves, so it can reduce a sealer's revocation (the reader sees
  that someone was lowered, not whom). A revoked editor, with the server, can
  hand such a reader a made-up key naming the head now: the reader writes
  content under nothing by a partial log, but relays its presence and cursors
  under it. Refusing such seals would refuse every honest epoch from an editor
  granted before any revocation
  (decisions/editors_are_served_whole_access_logs.md).
- **Stale heads on making.** A key minted on a device whose logs are stale
  names those stale heads: current only on devices behind as far.
- **An honest epoch from an administrator demoted since** cannot be verified
  by a device that holds the demotion, which stays on the older epoch it
  verified (or none, when the demotion outdates that one), warning
  `Substituted` or `Unverified`.
- **Forks.** A server that shows different devices different histories, with
  a revoked member's help, can read what a device it keeps on the old branch
  writes, and that device keeps taking records an old key signs after a
  retirement it never sees. It can also number a live session's operations
  differently for different participants
  (decisions/live_edits_are_ordered_by_the_server.md). The fix, not planned
  yet, is fork detection: signed log heads carried in sessions, relays and
  commits, compared when two devices meet. A block's signed log already has
  them per device: each device keeps the log's head it verified and takes a
  later one only with a proof that it extends it (`ClientError::LogForked`;
  guides/blocks/client.md, "Signed logs"), so a cut or rewritten log is caught
  by every device that saw the head, though two devices shown different logs
  learn of it only by comparing heads. For a session, each seal would name
  the last sequence it holds and a signed digest of the order up to it (each
  entry's sequence, floor and arrival as the server assigned them, its OpId,
  base and payload as its author sent them), so a participant that applied a
  different order reports the fork.
- **Stale authority.** A grant names the organization's or an ancestor's log
  as of an entry of its signer's choosing (`Authority`, `Link`); the server
  takes it only at that log's head, but one colluding with a demoted
  administrator, or a manager since revoked above, can append one naming an
  entry from before, and devices take it. A record resting on a block no
  longer above anything the reader views does not check for that reader until
  a manager signs it again from where it lies.
- **What a parent record proves on a device**: that its signer held Edit or
  above on the moved block. Edit on the destination, and Manage where
  `BlockGraph::move_refusal` asks for it, are the server's checks alone, so an
  editor, with the server, can place a block under one they manage and take
  Manage of it: they could already read it and leak its keys.
- **The server's tree.** Which blocks lie under a block is the server's
  listing (`children`, key work's rotations): a device wraps nothing under a
  scope the signed parent records do not reach, but the server can leave a
  block out of a rotation or a move's rewrapping.
- **A member roots a block an administrator never pinned.** An administrator
  who reaches a block only through escrow takes its root from a member's wrap,
  weakly (`RootPin::Weak`), so a member, with the server, can show it a
  made-up log and content as that block, shown unverified; the block's own
  members are not fooled. An administrator cannot re-wrap a key it took
  weakly, so after an escrow rotation such a block is escrowed again only by
  its own members' devices (key work's `unescrowed`); a stretch of unescrowed
  keys after a rotation is that, not a fault. The fix considered, not made:
  each block's root registered in the organization's log as it is made, the
  first claim winning, at an organization-log entry per block. An
  administrator's own escrow wrap is taken without checking the block's log,
  so a malicious administrator can root a block another reaches only through
  escrow; it reads everything through escrow anyway.
- **Authorship** is the root's and the owner records'. A transfer needs no
  acceptance ("Owner records").
- **A Join member's root.** A `Join` member takes a block's root from the log
  as served once a grant to it there is signed by a verified contact ("Hosted
  channels"), so that contact, with the server, can show it a log of their own
  making and stand in a host of theirs, as a contact can hand a device new to a
  block a key of it naming any heads. A member checks the host on a proof
  path, which can hide a later revocation of the host's Edit behind a hash
  (plans/hosted_blocks.md, "Accepted limits").
- **A leaked phrase** acts as the account until a retirement lands: it reads
  and writes what the account may and signs records, which stay valid after.
  The thief who retires first wins those logs, and a retirement's endorsement
  is bound to no time, so an old phrase can be retired in any log where it is
  still current.
- **Two key changes from one old phrase race.** Whoever holds an old phrase
  can leave a contact a key change of their own, sealed with the old key as
  the owner's is. A contact's device follows the first its mailbox holds; the
  other then opens from no key it holds and breaks the chain, so the contact
  is listed as not confirmed, with nothing sealed to them, until the two
  connect again by a link. Like a retirement, the change is bound to no time,
  so a thief who sends first is followed first.
- **Secrets at rest.** A device keeps the identity secret in plain text beside
  its vault and its token (`be_host_base::host_state`, `identity_secrets`), with
  no operating-system keystore, so whoever takes the device unlocked or its
  disk has the account. The server keeps its own account's secret plain in its
  database, so a backup together with the backup key opens the settings keys
  sealed to the server, the storage credentials and every push `auth` secret
  (guides/hosting.md).
- **A new phrase grows with the account.** `set-recovery-key` carries one
  retirement (300 to 400 bytes) per log the account signed in, so past roughly
  20,000 blocks it no longer fits in one frame (`MAX_FRAME_BYTES`) and the
  account cannot take a new phrase. Not fixed yet (plans/backlog.md, 18).

## Contacts

Every sharing relationship starts with a link passed outside the host: a grant
names only a contact of its granter (the server refuses any other), with the
keys the granter verified that way, which every device then seals to.

- A **contact card** (`be_block_keys::ContactCard`: account id, identity public
  key, signing key, name, email) is sealed with a link's code (`ContactCard::seal`, keyed by
  code and side). A contact link is `{server}/#contact/{CODE}`. What the account
  sends itself through the server (a record or a pin in a letter) is sealed with
  a key derived from its identity secret, under a context per kind
  (`RecoverySecret::seal_own`, `be_block_keys::Own`), so one never opens as
  another: sealing to the public key would let the server forge one the owner
  trusts.
- **The account's block** (`be_block::account_block`, `AccountBlock`) holds the
  account's records of its contacts, the code and kind of each link it made
  until it is finished (`contacts::KeptLink`, kept before `create-link`, so a
  code is never lost; an invitation's goes 30 days after it was made, and past
  `MAX_KEPT_LINKS` the oldest), and what it pins per organization: an owner-only block (guides/settings.md,
  Owner only) at the top of the personal organization, at an id derived from
  the account, made by its first device that has a record to keep
  (`be_block_client::AccountRecords`, whose connection keeps the personal
  organization and the block's key rows for the next pass: a peer of the
  personal organization that
  reaches this block alone and keeps nothing on the device). The server makes
  it only there, owner-only, of its type (`require_account_block`), and keeps
  of contacts only that two accounts are (`contacts`, two columns): no call
  lists them and nothing it holds names a contact's keys. An account in no
  personal organization (an agent) has no block, and keeps the letters its
  records arrive in. It follows its contacts' key changes from them on every
  read, so once its maker takes more than `MAX_LETTERS_FROM_ONE` new phrases
  the oldest change goes and the chain breaks for good: the maker makes the
  agent again.
- **A contact's record** (`contacts::OwnContact`) is the card the account
  verified, `verified`, a sequence set when it verified the card or followed a
  key change, and `through`, the newest mailbox message it reflects. Records
  arrive as letters sealed by the account for itself (`letter.contact`, left by
  the write that made two contacts: `accept-link` for the accepter,
  `create-agent` for both) and go as `letter.removed` (left to both by
  `remove-contact`); each pass of `contacts::contacts` takes them into the
  block in mailbox order, then deletes them, and a letter up to a record's
  `through` is taken already, so a device that stops between the two never
  takes back a removal nor follows a change twice. A removal is kept as such
  (`contacts::KeptContact::Removed`, with the newest `verified` and `through`
  the record had reached), so a record letter the server replays afterwards is
  taken only if newer, as a new link's always is; `remove-contact` leaves a
  record letter not yet taken ahead of the removal, so one pass that takes both
  keeps the removal with that record. The server cannot read or
  make a record, but it could serve an older version of the block to bring
  back a key the contact replaced, so each device pins the newest record of
  each contact it has seen (refs' `Contacts`), refuses an older one, keeps
  sealing to the pinned key, and says which (`contacts::Mismatch`: an older
  key, or a block it can't read, as after a new phrase taken on another
  device) while the block holds it. Key changes are followed from the record
  kept, in mailbox order, and deleted once the record that follows them is
  written, with any before the last followed that did not open (forged). One
  after it that does not open breaks the chain: the server let older ones go
  past `MAX_LETTERS_FROM_ONE`, or it is sealed to a phrase this account took
  since. The contact is then listed as not confirmed, named as verified, with
  nothing sealed to it, and the changes stay until the two connect again by a
  link, whose write deletes those that lead nowhere from the new card. Nothing
  is written or deleted while the block does not open on this device, since the
  changes may be sealed to a new phrase it lacks. A device writes the block
  only over the head it read (`AccountRecords::save`), so a device that read
  fewer changes than another never rolls a record back past a change the other
  already deleted: refused, it reads again. A contact who connects again by a
  new link, as one who lost their phrase must, gets a record that follows the
  newest the block holds, so it outranks every device's pin whatever their
  clocks say, and a new key needs no signed change.
- **A contact's name and email** are the card's, as verified, never the
  server's account row, which could name one contact as another so a share to
  "Bob" sealed to Mallory. They are as the contact gave them in their link;
  a contact who renames later keeps the old name until they connect again
  (plans/backlog.md, 18). A contact whose card has not opened shows the row's,
  marked as not confirmed, and nothing is sealed to it.
- **Links** are one family (`contacts.wit`): a card sealed with the code, a
  lookup id derived from it, and an accept that makes the two accounts
  contacts. Its `link-kind` says what else accepting does: a `contact` link may
  carry a pending grant (below); an `invitation` joins its organization as
  an editor. Any account makes a contact link; only an administrator of the
  organization makes an invitation (`rule` splits `create-link` on the kind),
  and any account opens and accepts it. No call lists an account's links: its
  block keeps their codes, and `delete-link` revokes one, or finishes one
  accepted; an invitation's row stays for the invite list.
- **The exchange** (`be_block_client::contacts`): the maker posts
  `create-link` (`make_link`, `invite`); the other side reads it
  (`open_link`), opens the card, and on "Connect with NAME (EMAIL)?" (or the
  invitation's join dialog) accepts with its own card (`accept_link`). The
  server records both `contacts` rows, leaves the accepter's card in the
  maker's mailbox (`letter.accepted`) and the accepter's record of the maker in
  the accepter's (`letter.contact`), and tells both; the maker's device then
  opens the card with the code it holds, keeps its record in the account's
  block (`through` the letter), signs a contact
  link's pending grant into the block's log (only while that log names the
  accepter nowhere, so finishing again never undoes a later revocation), and
  only then deletes the
  link and the message in one write, so a device that stops halfway finishes
  it again (`finish_links`, on connect and `MailboxChanged`).
- **A pending grant**: a link may carry a block, an access level and the
  block's scope key sealed with the code (`Peer::offer`). Accepting applies the
  grant at once, so a share works while the maker is offline; finishing
  reseals the scope (`Peer::seal_share`). A grant to a contact whose key the
  granter has not verified yet works on the server at once and is sealed later
  through `unsealed`.
- `RemoveContact` deletes both rows, what each left the other and the links
  between them, leaves each a `letter.removed`, and revokes each one's grants on
  blocks the other may edit, in every shared organization. Contacts are per account,
  across organizations; a grant also needs membership.
- **In the host**: the Contacts dialog, the share dialog (whose "Invite someone
  new" calls host-services' `make-contact-link`), and the worker's
  `Shared::contacts`, the only keys a share is sealed to.

## What the server can and cannot do

It stores and relays everything, so it can withhold, delay, replay or delete
rows, refuse requests, and see who shares with whom, block shapes, sizes and
timing. It cannot read content or metadata, and it cannot choose a key a device
will seal with: keys sealed to an account are sender-authenticated and taken
only from verified contacts, or from members the organization's log names
(their keys as it names them) whom the block's log grants Edit or above; a contact's key changes only by a change sealed
with the verified key or a newer record of the account's own, never back to
an older one, and a contact's name is the one it verified; cards, codes, pending grants and invitation escrow keys
are sealed with codes it never sees; a device wraps only under keys it trusts;
escrow keys are used only when pinned or signed down from a pinned one. It
cannot sign as a contact: a contact's signing key comes from its card or a key
change. It cannot sign anyone into an organization: a person signs in with the
key of an invitation an administrator opened, derived from a code it never
sees, once; it still decides when an invitation expires, and can list as a
member someone no record names, whom devices then treat as no member
(`unsigned-members`). A member record it makes up is caught only by whoever is
served the organization's whole log (administrators, and members with Edit or
above anywhere): a reader below Edit is served a path that reduces other
members' records, so it cannot tell a forged member from a real one, though it
seals nothing to either.

It can make an account forget a contact, by a made-up `letter.removed`,
which connecting again by a link recovers. What it cannot do is make the
account take back an older card than one it held: a removal is kept with the
newest record it ended. That is the guarantee, not that a removal never comes
back. A contact made and removed before any device of the account took the
record is the case left: a server that withholds the `letter.contact` until
after the `letter.removed` leaves a tombstone that never saw the record, so a
device takes the record when it arrives, and the removed contact comes back,
with the card the account itself accepted.

A contact's key changes reach a device only through the mailbox the server
keeps, so it can also withhold or drop them, or put one there that does not
open: the device then finds the contact's chain broken and lists them as not
confirmed, sealing nothing to them and taking what their verified keys sign as
a replaced key's, until a new link. That is a downgrade to unverified, which
the server could already cause by withholding; it cannot forge a change, nor
hide one beyond withholding it.

It still decides, and devices trust it for: which held key is a block's current
data key and which epoch is newest, among the rows its keys name, since a wrap
names its own row and so cannot pass an old key off as a new epoch. A server
colluding with someone who lost access, or with a demoted administrator, can
therefore keep a device writing under a key that person knows, for as long as
it withholds from that device what outdates it. It also decides:
which blocks lie under a block, as listings and key work name
them (a device wraps nothing under a scope its block's signed parent records do
not reach, but may leave a block out of a listing or a rotation), and who views
a block inside, for a kind opened to anyone inside; the log's head, up to the heads an
epoch names (a server that withholds a revocation from every device that holds
an epoch made under a head before it keeps that epoch current for them; a
made-up key from a contact is taken as their Edit, or administering, as of a
head they choose among those the log holds, never for an escrow key; and a
key minted on a device whose logs are stale names those stale heads;
"Accepted limits"); what an escrow-only administrator, or a grantee
who is no contact of the sender, sees of a block whose root only a member's
wrap carried (a member and the server together can show it a made-up log and
content, shown unverified: `RootPin::Weak`); whether a reader below Edit,
served a proof path, sees a sealer's revocation (it sees only that someone
was lowered); which blocks an account knows exist;
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
