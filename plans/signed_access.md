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

- **Root.** A block's creator is fixed by its id: `CreateBlock` takes an id
  derived from the creator's signing key and a random nonce, as version-8 ids
  already mark owner-only keys (`be_block_protocol::owner_only_key`), and
  carries the nonce. Anyone holding the id checks the creator's key against
  it, so the server cannot swap the root. An organization's id is derived the
  same way from its creator's.
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
by the root. Each block's records form one log whose entries name the hash of
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
2. **Roots.** Derived block and organization ids, checked on every summary a
   device takes. Done: a summary whose creator does not match its id is
   refused by the client, shown by a test with a forging server.
3. **Signed grants and administrators.** Records, logs, the server's checks
   and the devices'. Done: a test where the server lists a contact as granted
   with no record and no key reaches them; one where a revoked grant's record
   is withheld and the device refuses to roll back below its pinned head.
4. **Signed parents.** Done: a test where the server claims a block moved under
   a scope a contact manages and the next rotation wraps nothing under it.
5. **Guides.** guides/blocks/keys.md's server section and
   guides/blocks/server.md's access section describe the records.

## Accepted limits

- The server can withhold records it has not yet shown a device: a revocation
  it hides keeps the revoked account's grant alive to devices that never saw
  it, until a device that did tends the block. Only the revoked person gains,
  and only with the server's help.
- `Edit` and `View` hold the same keys; Edit stays enforced by the server, so
  a server granted View can write
  (decisions/the_server_never_vouches_for_an_identity.md).

## Open questions

- **Q1. The root.** Derive block and organization ids from the creator's key
  (above), or record the creator in a signed first log entry that devices pin
  the first time they see the block? Deriving lets anyone who has only the id,
  such as a Join member, check the chain with nothing pinned; it costs 16-byte
  ids built from a hash and nonce instead of random ones.
- **Q2. A creator's new phrase.** A new recovery phrase changes the signing
  key the root names. Follow the key change chain contacts already follow
  (`contacts::new_identity`), so the old root still checks through it, or
  require the creator to re-sign the block's log with the new key?
- **Q3. Withheld revocations.** Is the limit above acceptable, or should a
  revocation also need a signed acknowledgement from a second manager or the
  revoked account before devices treat the grant as live?
