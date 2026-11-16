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
  making them, so no contact check is needed; the creator picks the agent's
  account id so the grant can name it, and `create-agent` carries the grant.
  A grant also names where its signer's `Manage` comes from (`Authority`):
  the log itself, the organization's log as of one of its entries
  (administering it), or a block above as of an entry of its log. A log then
  checks the same however the other log changes later: a grant an
  administrator signed still checks after they are demoted.
  Grants to anyone inside and to the server's own account are standing
  exceptions, server rows with no record: no device seals to either by grant.
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

## Grants the server changes first

Deliberate: the server may withhold at once, but never adds access a device
did not sign.

- **A contact link's grant.** The maker does not know who will accept, so no
  grant can name them. Accepting applies the server row at once and the scope
  key reaches the accepter sealed with the link's code; the maker's device
  signs the grant when it finishes the link (`finish_links`, once it opens the
  accepter's card). Window: until then the log does not name the accepter, so
  no other device seals them a later epoch, and they read only the epochs the
  link carried.
- **`remove-contact` and `remove-member`** revoke the server rows at once,
  inside the server's transaction. The signed revocations follow from a device
  (the remover's, or an administrator's), listed in key work as grants the
  log names that the server revoked. Window: until then the log still names
  the grant; a device sealing by the log also checks the server's row, so
  nobody seals them a new epoch meanwhile, and the rotation key work asks for
  goes ahead.
- **Standing grants**: to anyone inside (a kind of settings a viewer inside
  offers its contacts) and to the server's own account (the settings scope,
  crypto audit HIGH 1) are rows with no record. Neither is ever a sealing
  target by grant: a device seals only to grantees a log names, and no record
  can name the server, which only ever holds the organization settings'
  scope key. Window: none beyond those two rows.

## Accepted limits

- What a revoked member already held stays readable to them: the keys of
  every epoch before the rotation.
- `Edit` and `View` hold the same keys; Edit stays enforced by the server, so
  a server granted View can write
  (decisions/the_server_never_vouches_for_an_identity.md).
- **Forks.** A server that shows different devices different histories, with
  a revoked member's help, can read what a device it keeps on the old branch
  writes, and that device keeps accepting records the old key signs after a
  retirement it never sees. The same server can also fork a live session's
  order, numbering a block's operations differently for different
  participants (decisions/live_edits_are_ordered_by_the_server.md), so their
  copies part. The fix, not planned yet, is fork detection: signed log heads
  carried in sessions, relays and commits, so two devices that meet compare
  what they were shown. The heads carried cover the session order too: each
  seal names the last sequence it holds and a digest of the order up to it,
  signed by the holder, carried in the seal's sealed commit state as well as
  in the relayed `Sealed`. The digest covers what the server assigned to each
  entry (its sequence, floor and arrival) and what its author sent (its OpId,
  base and payload), so a server that renumbers, restamps or reorders entries
  is caught, not only one that alters them; a participant that applied a
  different order through that sequence reports the fork. A participant who
  leaves before the next seal is not checked against it.
- A reader below Edit checks a sealer on a proof path, which can hide a later
  revocation of the sealer. That only lets a revoked editor hand a reader a
  key, and a reader below Edit never seals or wraps with it.
- **A leaked phrase** lets whoever holds it act as the account until it
  retires the key: read and write what it may, sign records, host a hosted
  block it has Edit on, or join one as a player.
- **Retirement races.** A thief holding the old phrase can sign records until
  the retirement lands, and those stay valid after it. If the thief takes a
  new phrase first, the thief's retirement is the one in the logs and the
  owner is locked out, as with a stolen phrase today. A retirement's
  endorsement is bound to no time or epoch, so whoever holds an old phrase can
  retire it, at any later moment, in any log where it is still current.
- **A stale authority.** A grant names the organization's or an ancestor's
  log as of an entry of the signer's choosing. The server checks it is that
  log's head when it appends the grant, but a server colluding with a demoted
  administrator (or a manager since revoked above) can append one naming an
  entry from before the demotion, and devices take it. A retired key still
  signs nothing after its retirement in the log the grant goes in. Escrow
  epochs are not open to this: a device takes one only from an administrator
  as of the organization log's head it holds.
- **Ancestors' roots** are taken from the server when this device pinned none
  (a grant on a child, from a manager above), as the tree they hang in is the
  server's until parents are signed (step 4). So the server, with anyone
  managing any block, can make a grant naming `Inherited` authority over an
  unrelated block check. 3c therefore seals to no grantee whose grant (or
  whose granter's) rests on `Inherited` authority until step 4 lands.
- **A member roots a block an administrator never pinned.** An escrow wrap
  pins the root it carries when its sender is a member with Edit or above in
  the log rooted there, so a real member, with the server, can root a block
  id the administrator has not pinned (one only ever reached through
  escrow) at themselves and show a made-up log and content as that block.
  An escrow-only administrator's view of a member's block is only as good
  as the member who sent it, and is shown as such: the root is weak (3a),
  passed on to no one and sealed by never, and a genuine wrap carrying the
  real root then shows a conflict. The block's own members, who pinned its
  real root, are not fooled. An administrator cannot re-wrap a key it took
  weakly, so after an escrow rotation a member's block is wrapped under the
  new epoch only once its own members' devices wrap it (key work's
  `unescrowed`), not by the administrator who reached it through escrow;
  a stretch of unescrowed keys after a rotation is that, not a fault. The
  full fix considered: anchor block roots in
  the organization's log, a member's record of each block's root signed on
  making it, the first claim winning, so a carried root must match the one
  registered. It costs an organization-log entry per block made, and that
  log is served whole to members with Edit or above.
- **An administrator's carried root is taken for a block the device never
  pinned.** An
  administrator's escrow wrap is taken without checking the block's log
  (administrators re-wrap every key under each new escrow epoch), so a
  malicious administrator can root a block id another administrator reaches
  only through escrow at themselves. An administrator reads everything
  through escrow anyway.
- **Authorship** is the root's (and later owner records'): no grant names or
  lowers a block's creator.
- **A new phrase grows with the account.** `set-recovery-key` carries one
  retirement (about 300 to 400 bytes) per log the account signed in, which is
  every block it ever made, beside the resealed keys that already grow the same
  way, and `get-recovery-key` decodes each of those logs to serve its head.
  Past roughly 20,000 blocks a new phrase no longer fits in one frame
  (`MAX_FRAME_BYTES`), which would lock a heavy user out of the one answer to
  a stolen phrase. Not fixed yet: either send the change in parts committed by
  a last call that keeps the all-or-nothing check, or keep one account-level
  keys log holding the retirement once, which a block's check looks the
  signer's current key up in, so a change costs one entry whatever the number
  of blocks. The test that shows it: an account with more blocks than one
  frame's worth of retirements takes a new phrase, and every log is retired.

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
3. **Signed grants and administrators**, in five pieces, each reviewed on its
   own. Inherited access keeps trusting the server's tree until step 4 signs
   parents, and a block's agent keeps reading its organization's log
   (`bound_may`), since a terminal host opens its organization as
   `Peer::attach` does.
   - **3b. Grant and administrator records.** Grants (revocation is a grant
     to `None`) in a block's log and administrators in the organization's,
     each checked at its place in the log (`check_log`) by the server and by
     devices; `set-access` and `set-role` carry the signed record. An agent's
     access comes only from a grant its creator signed (crypto audit 9, the
     agents part). Escrow epochs are bound to the administrator records, so a
     demoted administrator cannot sign the next one (crypto audit 10). Done: a
     test where a forged administrator record is refused; one where a
     demoted administrator's escrow epoch is refused. Grant changes the server
     makes on its own stay server rows, signed after by a device (built with
     3c, which is what seals by the records); see "Grants the server changes
     first".
   - **3a-members. Members in the organization's log.** A Member record
     (account and keys, member or not, person or agent) signed by an
     administrator: an administrator's device signs a person in when it
     finishes the invitation it made, as `finish_links` finishes a link (the
     code is the evidence; that someone else is a member is the server's
     word alone, so other unsigned members wait, listed, for an
     administrator to act). An agent is signed in by its maker, a member,
     at `create-agent`; its account id is derived from its maker and its
     keys (`agent_account`), so no member can claim a person's account as an
     agent. No one already named is signed in again: keys change by the
     member's own retirement, or by signing out and in. Removing a member
     signs them out, and a maker's `tend` signs out its agents the server
     removed on its own (collected with their block, or deleted on taking
     an account back), so those stay named one `tend` at most. This anchors
     an escrow wrap's sender (3a) where the server cannot: an administrator
     reaching a block only through escrow has pinned no root for it, so a
     log rooted at the root the wrap carries proves nothing about a sender
     alone. Until an administrator's device signs a joiner in, that member's
     escrow wraps wait unopened; key work lists such members to
     administrators, and to the member, so a stuck invitation shows. Done: a
     test where the server adds a member no administrator signed, and key
     work lists them, to administrators and to themselves, and no
     administrator's device signs them in (3a adds that their wrap stays
     unopened); a unit test where a member claiming a person's account as
     its agent is refused.
   - **3a. Authenticated escrow wraps**, after 3a-members, whose records name the
     keys an administrator checks a sender by: an escrow wrap is sealed by its
     sender to the escrow key, as `seal_for` seals to an account, not
     anonymously, so the server cannot make one (an anonymous wrap lets
     anyone, the server included, seal a made-up key of a block to the escrow
     key and show administrators forged content of it; the server claiming a
     block is under the organization's settings does the same, since a key
     the server holds reads there, plans/keys_bound_to_blocks.md), and is
     opened only from a
     sender the organization's log names, with the keys it names: an
     administrator (who holds the escrow key, so reads all it opens anyway,
     and re-wraps under each new epoch), or a member the block's log (rooted
     at the root this device pinned, or else the one the wrap carries) names
     with Edit or above; the root it carries is then pinned. A root taken
     only from a member's wrap, for a block this device pinned nothing else
     for, is pinned weakly (`RootPin::Weak`): shown as unverified, never
     passed on, its key untrusted (it vouches for nothing it opens), and
     pinned in full only once a trusted key or an administrator carries the
     same root. A wrap carrying any other root than the one held marks the
     block's root conflicting, shown as forged, whichever came first. Sealing it from
     the sender sooner would prove nothing, as a sender who is not a contact
     has only keys the server names. Done: a test where a wrap the
     server seals to the escrow carries no root anyone trusts.
   - **3c. Sealing to whom the log names.** `tend` seals a scope key only to
     grantees checked in the logs up the ancestry, with the keys their grant
     names, contacts or not, and never an owner-only block's keys to anyone
     but its author (crypto audit 6, the `unsealed` part); a pin is taken only
     from a sender holding Edit or above as of the log's head, and a key from
     any such sender. It never seals by a log checked from a weak root
     (3a). Built: a scope key is sealed only by its block's own log (grants
     on an ancestor reach a child through the ancestor's scope wrap, so
     nothing is lost before step 4 signs parents), never on a grant resting
     on `Authority::Inherited` directly or through its granters
     (`Checked::tainted`); the escrow and settings keys go to
     administrators the organization's log names. A key sealed by a member
     who is no contact opens with the keys the organization's log names,
     weakly and untrusted (3a's rules). Removals made from a device
     (`remove-member`, `remove-contact`, revoking in the share dialog) carry
     their signed revocations, refused without them; only the server's own
     (agents collected or deleted, `remove-contact` in another organization)
     wait in key work for a manager's `tend`. Left to 3d: sealing against an
     epoch's head (until then a server that withholds a signed revocation
     can have the next epoch sealed to the revoked member), and checking a
     contact sender's Edit as of that head. Done: a test where a grantee who is not a contact gets
     the key and a contact with no grant does not; one where the server lists
     a contact as granted with no record and no key reaches them.
   - **3d. Key epochs bound to log heads.** Each new epoch names the log head
     it was made under, signed by its maker; a device seals or wraps it only
     to members granted as of that head and must hold the log up to it. Done:
     a test where the server withholds a revocation from a device and that
     device seals the new epoch to nobody until it has the log up to the
     epoch's head.
   - **3e. Proof paths.** Whole logs to Edit and above, proof paths to
     readers below Edit ("Who is served what"). Done: a test where a member
     with View is served no other member's grant.
4. **Signed parents.** Done: a test where the server claims a block moved under
   a scope a contact manages and the next rotation wraps nothing under it.
5. **Owner transfer.** Owner records, and transferring ownership in the shell.
   Done: a test where ownership moves and the old owner can no longer grant.
6. **Guides.** guides/blocks/keys.md's server section, and its rule that keys
   are sealed only to and taken only from verified contacts, which a checked
   grant now also satisfies; guides/blocks/server.md's access section.
