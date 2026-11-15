# Signed access

Depends on: none.

Today the server decides who may have a block's keys, and devices act on what it
says (guides/blocks/keys.md, "What the server can and cannot do"): a device
seals a scope key to any verified contact the server lists as granted (key
work's `unsealed`), and the escrow key to any verified contact it lists as an
administrator. The server cannot make up a person, but it can name a real
contact who was never granted, so the server and any one contact of an
editor can read a block. The rule this plan holds the stack to
(decisions/the_server_never_vouches_for_an_identity.md): with `None` or
`KnowExists`, the server can withhold or destroy, never read or write, alone
or with anyone the block's managers did not choose, apart from the standing
exceptions that decision names.

So every change to who may do what is signed by the member who made it, and
devices act only on signed records they can check back to the block's
creator. plans/hosted_blocks.md and plans/share_links.md build on these
records.

## The records

All are signed with the account signing key (`RecoverySecret::signing`) in a
domain of their own and kept by the server in the clear (it already knows who
is granted what).

- **Root.** The first entry of a block's log names its creator and their keys
  and is signed by them. The log is in the clear, so the server could replace
  it whole with one rooted at a creator it made up; a device therefore takes
  the root's hash only from somewhere the server cannot write: making the
  block itself, a grant a verified contact sealed to it (the hash travels
  beside the key), or a share link's secret part (plans/share_links.md).
  Later entries chain from it. Block ids stay random.
- **Organization root.** A company organization has a log of its own, rooted
  at its creator when `create-organization` makes it. A member pins its hash
  from the invitation, which carries it sealed with the code beside the escrow
  key (guides/blocks/keys.md, "Inviting"). The personal organization has no
  administrators and so no log.
- **Owner.** New owner account and keys, signed by the current owner:
  ownership moves to another member. The owner holds what authorship gives
  today (`Manage` while they can view the parent).
- **Grant.** Grantee account, the grantee's X25519 identity key and signing
  key as the granter verified them, and the level. Revocation is a grant to
  `None`; a member revoking their own grant (as anyone may today) signs it
  themselves. A block's agent is granted by its creator, who holds `Manage`
  (`create-agent`'s `bound-to` needs it) and names the agent's keys from
  making them, so no contact check is needed.
- **Parent.** New parent and the hash of the parent's log root, signed by the
  member who moved it. Inherited access is checked only through signed parent
  records, so the server cannot place a block under an ancestor someone else
  manages, nor serve a made-up log for a real ancestor, rooted at a creator it
  controls.
- **Administrator.** Account, keys, administrator or not, signed by an
  administrator, in the organization's log.

Each record names the hash of the entry before it, so its place in the log
orders it; there are no separate sequence numbers. A record is valid when its
signer held the level it needs at that place: `Manage` to grant or revoke
another member, `Edit` on both ends to move (and `Manage` where
`BlockGraph::move_refusal` asks for it), an administrator to change
administrators. Authorship counts as `Manage`, proven by the root or the
latest owner record. The server can withhold the newest entries but cannot
reorder or drop one from the middle; a device pins the newest log head it has
seen.

## Who is served what

The whole log of a block goes to its members with `Edit` and up, and so do
the whole logs of every ancestor up to the root. They are the
ones who seal (key work needs only Edit, and they read all the content
anyway), and a sealer must see every entry up to the head: an entry reduced to
its hash could be the revocation of the person it is about to seal to, or of
whoever granted them. This reveals little: everyone granted on an ancestor
already has access to this block (access is the highest grant on the path
up), and its managers already see them (`list-access`). What an editor newly
learns is that set at Edit too, the ancestors' history (grants since
revoked), and their parent and link records. Accepted, as simpler than a
commitment to the log's state that would let a sealer check one member
without the rest. Not to
`KnowExists`: anyone gets that by listing an id in their own block's
references (guides/blocks/server.md, "Organizations and access"), and the log
names every grantee.

Likewise, members with `Edit` or above on any block of a company organization
are served the organization's whole log, not a proof path: a sealer checking a
grant an administrator signed must know the signer was not demoted before it,
and a proof path could hide that demotion. It reveals only who is or was an
administrator.

Readers below Edit (View, and Join in plans/hosted_blocks.md), who never
seal, are served **proof paths**: the entries that prove one right, and the
chain they hang from, and nothing about other members.

- **A direct grant**: the root, the grant, and the log entries between them,
  each reduced to its hash where the reader needs only the chain (the signed
  entry it names and the hash of the one before).
- **An inherited right**: the signed parent records from the block up to the
  ancestor that grants it, and that ancestor's proof path for the one grant.
  Siblings and the ancestors' other members are not on it.
- **Authorship**: the root or the latest owner record, and the proof that the
  author can view the parent.
- **An administrator**, for a reader below Edit: the organization log's path
  from its root to the record that made them one.

A reader below Edit is served the path of its own grant. Hosted blocks add
the host's grant and bans (plans/hosted_blocks.md). A proof path can hide a
later revocation behind a hash; for a reader who never seals, that only
matters for a hosted block's host (plans/hosted_blocks.md, "Accepted
limits").

## Sealing to someone who is not a contact

Today a key sealed to an account is taken only from a verified contact, and a
device seals only to one. A checked grant now stands in for that contact check
(decisions/share_links_are_an_out_of_band_start.md): it names the grantee's
X25519 and signing keys as a member with the right level verified them, so a
device may seal to a grantee it has no contact row for, using the keys in the
grant. A receiving device accepts a key sealed by any sender holding `Edit`
or above as of the epoch's head, not only by a contact: key work is done by
whichever editor runs `tend`, which is often not the receiver's granter. An
editor receiving it checks the sender in the whole log it holds; a reader
below Edit is served the sender's proof path beside the sealed key. `get-signing-keys` stays only for showing authors
(`Signer::Unverified`).

## What changes

- **The server** stores the logs and checks each record as devices will,
  refusing one that does not check, so a good server and a device never
  disagree. `set-access`, `set-parent`, `create-block` and `set-role`
  (`be-block-protocol/wit/`) carry the signed record instead of the bare
  request; `create-organization` carries the organization root. `BlockGraph`
  keeps working from the records as it does from the rows today.
- **Devices** check the log up to its head before they seal or wrap (the audit
  below), and a row that does not check is skipped as a forged one is today.
  The shell's access list shows only checked grants.

## Key epochs and withheld revocations

A device knows the log only as far as the server has shown it. If Alice
revokes Bob on her laptop, the server can keep showing Carol's phone the log
without that entry, and Carol's phone, doing key work, would seal the next key
to Bob. So every new key epoch names the log head it was made under, signed
with the account key of whoever made it; a device seals or wraps an epoch
only to members granted as of that head or later, and must hold the whole
log up to that head to check it. A revocation is always followed
by a rotation, whose new epoch names a head at or after the revocation, so no
device that sees that epoch can seal it to Bob.

This stops a server that only withholds. It does not stop one that **forks**
the log, showing Carol's phone a history that never reaches the revocation or
the rotation: Carol's phone keeps writing under the old epoch, which Bob
holds (Accepted limits).

## A new recovery phrase

An account's signing key is derived from its phrase, and someone who thinks
theirs leaked takes a new one (guides/blocks/keys.md, "Identity, new phrases
and devices"). Records signed with the old key stay valid, but whoever holds
the old phrase could sign new ones. So taking a new phrase appends a
**retirement** entry, signed with the new key and carrying the key change, to
the log of every block where the account holds a signed record, and to its
organization's log; that log refuses old-key records after it. A device that
has not seen the retirement is covered as for a withheld revocation, and not
covered against a fork. plans/authorship.md has the same gap for what an
account signs inside blocks ("A replaced key still verifies what it
signed").

## Accepted limits

- What a revoked member already held stays readable to them: the keys of
  every epoch before the rotation.
- `Edit` and `View` hold the same keys; Edit stays enforced by the server, so
  a server granted View can write
  (decisions/the_server_never_vouches_for_an_identity.md).
- **Forks.** A server that shows different devices different histories, with
  a revoked member's help, can read what a device it keeps on the old branch
  writes, and that device keeps accepting records the old key signs after a
  retirement it never sees. The fix, not planned yet, is fork detection:
  signed log heads carried in sessions, relays and commits, so two devices
  that meet compare what they were shown.
- A reader below Edit checks a sealer on a proof path, which can hide a later
  revocation of the sealer. That only lets a revoked editor hand a reader a
  key, and a reader below Edit never seals or wraps with it.
- **A leaked phrase** lets whoever holds it act as the account until it
  retires the key: read and write what it may, sign records, host a hosted
  block it has Edit on, or join one as a player.
- **Retirement races.** A thief holding the old phrase can sign records until
  the retirement lands, and those stay valid after it. If the thief takes a
  new phrase first, the thief's retirement is the one in the logs and the
  owner is locked out, as with a stolen phrase today.

## Audit

Every place a device seals, wraps or hands over a key, the server claim it
acts on today, and the step below that makes it act on a checked record
instead.

| Where | What it trusts the server for | Step |
|---|---|---|
| `Peer::tend`, `unsealed` (`peer/rotation.rs`) | that the account is granted the scope | 3 |
| `Peer::tend`, `unescrowed`, and `make_escrow`'s seal to each administrator (`members()`) | who administers the organization | 3 |
| `Peer::share` / `seal_share` (`peer.rs`) | nothing new: the sharer chooses the grantee; the record is signed | 3 |
| `Peer::offer` (pending grant in a contact link) | nothing: sealed with the link's code | 3 (signs its record) |
| `Peer::rotate` (`peer/rotation.rs`) | the subtree it walks (`children`), each block's parent and scope | 4 |
| `Peer::set_parent` (`peer.rs`) | the destination's scope and what is inside the moved block | 4 |
| `Peer::create` / `mint_scope`, `mint_data` (`keys.rs`) | the parent's scope key it wraps under | 4 |
| `KeyBook::absorb` | which block a key row is for, and which epoch is newest | 3 (epochs name heads) |
| `contacts::new_identity`, `reseal_from` (`ResealKeys`) | nothing: contacts and their verified keys | 2 (retirement entries) |
| `Peer::share_with_server` (organization settings) | the server's account key (`ServerKey`) | standing exception |
| `seal_notification` | nothing: sealed with a key the device holds | none |

## Order of work

1. **Audit.** Done: the table above.
2. **Roots and retirement.** Built (guides/blocks/keys.md, "Access logs" and
   "Identity, new phrases and devices"): block roots, pinned from the creating
   device and from every key (`CarriedKey`); organization roots, pinned from
   making the organization, the invitation and an agent's maker; and
   retirement entries on a new phrase, endorsed by the old key and signed by
   the new, in every log the account signed in. A root carried only by a key opened through escrow is not pinned
   (anyone can seal to the escrow key); step 3 decides how an administrator
   checks a log it reaches only through escrow. Agents' blocks and records are
   rooted at the agent's own keys, which its maker makes and holds no copy of,
   so only the agent (or a later owner record) retires or transfers them.
3. **Signed grants and administrators.** Records, logs, proof paths, the
   server's checks and the devices', sealing to grantees who are not contacts,
   and key epochs bound to log heads. Before any decision acts on a pin:
   authenticate escrow wraps (sealed by their sender, as `seal_for` is,
   rather than anonymously) or give administrators another check of a root
   they reach only through escrow, which today pins nothing; and take a pin
   only from a sender the log entitles to seal the key (today any verified
   contact can plant a block's first pin). Decide too whether a block's agent
   may read its organization's log: `bound_may` lets it today, since a
   terminal host opens its organization as `Peer::attach` does. Done: a test where the server lists a
   contact as granted with no record and no key reaches them; one where the
   server withholds a revocation from a device and that device seals the new
   epoch to nobody until it has the log up to the epoch's head; one where a
   member with View is served no other member's grant.
4. **Signed parents.** Done: a test where the server claims a block moved under
   a scope a contact manages and the next rotation wraps nothing under it.
5. **Owner transfer.** Owner records, and transferring ownership in the shell.
   Done: a test where ownership moves and the old owner can no longer grant.
6. **Guides.** guides/blocks/keys.md's server section, and its rule that keys
   are sealed only to and taken only from verified contacts, which a checked
   grant now also satisfies; guides/blocks/server.md's access section.
