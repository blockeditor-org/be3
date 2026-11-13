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
or with anyone the block's managers did not choose.

So every change to who may do what is signed by the member who made it, and
devices act only on signed records they can check back to the block's
creator. plans/hosted_blocks.md and plans/share_links.md build on these
records.

## The records

All are signed with the account signing key (`RecoverySecret::signing`) in a
domain of their own, kept by the server in the clear (it already knows who is
granted what) and served to anyone who may know the block exists.

- **Root.** The first entry of a block's log names its creator and is signed
  by them. The log is in the clear, so the server could replace it whole with
  one rooted at a creator it made up; a device therefore takes the root's hash
  only from somewhere the server cannot write: making the block itself, a
  grant a verified contact sealed to it (the hash travels beside the key), or
  a share link's secret part (plans/share_links.md). Later entries chain from
  it. Block ids stay random. An organization has a root the same way.
- **Owner.** Block, new owner account and key, signed by the current owner:
  ownership moves to another member. The owner holds what authorship gives
  today (`Manage` while they can view the parent).
- **Grant.** Block, grantee account and the grantee's signing key as the
  granter verified it, level, a sequence number, signed by the granter.
  Revocation is a grant to `None`.
- **Parent.** Block, new parent, sequence number, signed by the member who
  moved it. Inherited access is checked only through signed parent records,
  so the server cannot place a block under an ancestor someone else manages.
- **Administrator.** Organization, account, key, administrator or not,
  sequence number, signed by an administrator; the first is the organization's
  creator.

A record is valid when its signer held the level it needs at that record's
place in the block's log: `Manage` to grant or revoke, `Edit` on both ends to
move (and `Manage` where `BlockGraph::move_refusal` asks for it), an
administrator to change administrators. Authorship counts as `Manage`, proven
by the root or the latest owner record. Each block's records form one log whose entries name the hash of
the previous one, so the server can withhold the newest but cannot reorder or
drop an entry from the middle; a device pins the newest log head it has seen.

## What changes

- **The server** stores and serves the logs and checks each record as devices
  will, refusing one that does not check, so a good server and a device never
  disagree. `SetAccess`, `SetParent`, `CreateBlock` and the member calls carry
  the signed record instead of the bare request. `BlockGraph` keeps working
  from the records as it does from the rows today.
- **Devices** check the chain before they seal or wrap: `unsealed` key work,
  `Peer::share`, `Peer::set_parent`, `Peer::rotate` and escrow sealing act
  only on records that check, and a row that does not is skipped as a forged
  one is today. The shell's access list shows only checked grants.
- **Signing keys.** A grant names the grantee's key as the granter verified
  it, so a device checking a grant to a stranger needs no key from the
  server; `get-signing-keys` stays only for showing authors
  (`Signer::Unverified`).

## Order of work

1. **Audit.** List every place a device seals, wraps or hands over a key, and
   the server claim each acts on (grants, administrators, the subtree a
   rotation or move walks, which block a key row says it is for). Done: the
   list is in this plan, each entry marked with the step that removes it.
2. **Roots and owners.** Root and owner records, the root pinned from the
   creating device, sealed grants and links, and transferring ownership in
   the shell. Done: a test where a forging server
   swaps a block's root and the device refuses it; one where ownership moves
   and the old owner can no longer grant.
3. **Signed grants and administrators.** Records, logs, the server's checks
   and the devices', and key epochs bound to log heads (below). Done: a test
   where the server lists a contact as granted with no record and no key
   reaches them; one where the server withholds a revocation from a device and
   that device seals the new epoch to nobody until it has the log up to the
   epoch's head.
4. **Signed parents.** Done: a test where the server claims a block moved under
   a scope a contact manages and the next rotation wraps nothing under it.
5. **Guides.** guides/blocks/keys.md's server section and
   guides/blocks/server.md's access section describe the records.

## Revocations the server withholds

A device knows the log only as far as the server has shown it. If Alice
revokes Bob on her laptop, the server can keep showing Carol's phone the log
without that entry, and Carol's phone, doing key work, would seal the next key
to Bob. So every new key epoch names the log head it was made under, signed by
the device that made it; a device seals or wraps an epoch only to members
granted as of that head or later, and must hold the log up to that head to
check it. A revocation is always followed by a rotation, whose new epoch names
a head at or after the revocation, so no device can seal it to Bob without
having seen him revoked. Withholding then only delays sealing to people still
granted, which is denial, not reading.

## Accepted limits

- What a revoked member already held stays readable to them: the keys of
  every epoch before the rotation.
- `Edit` and `View` hold the same keys; Edit stays enforced by the server, so
  a server granted View can write
  (decisions/the_server_never_vouches_for_an_identity.md).

## A new recovery phrase

An account's signing key is derived from its phrase, and someone who thinks
theirs leaked takes a new one (guides/blocks/keys.md, "Identity, new phrases
and devices"). Records signed with the old key stay valid, but whoever holds
the old phrase could sign new ones. So taking a new phrase appends a
**retirement** entry, signed with the new key and carrying the key change, to
the log of every block where the account holds a signed record; that log
refuses old-key records after it. A device that never sees the retirement is
covered as for a withheld revocation: the rotation that follows names a head
at or after it.
