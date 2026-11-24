# An owner transfer waits for acceptance

A block's ownership moves in two owner records in its access log
(guides/blocks/keys.md, "Owner records"). First the owner offers the block to
another member who is their contact. The offer moves nothing. Then that member
accepts it, signing a record that names themselves with the keys the offer
named. Only the acceptance makes them the owner and leaves the former owner
Edit. The owner may withdraw an offer by offering the block to themselves, or
replace it by offering the block to someone else.

An offer ends with the access of the account it was made to. A revocation
below View, a grant naming other keys, or a ban ends it in the log. Access
taken away on a block above is not in this log, so the server refuses an
acceptance, and stops listing the offer, once the account can no longer read
the block. A new phrase moves the offer to the account's new keys, so a stolen
old phrase cannot accept it.

## Why

Ownership is responsibility as well as power. The new owner can leave the
block only by giving it on, since no grant names or lowers an owner, and the
block counts against their bound on blocks. Nobody should have that put on
them without saying yes. An acceptance signed with the keys the offer named
also shows that the account holding those keys is the one taking the block.

## Rejected

- **A transfer that takes effect at once**, as owner records first worked: it
  needs one record and no answer, but it hands a block, and its place in the
  recipient's quota, to someone who never asked for it.
- **An offer that outlives its account's access**, checked only against the
  keys the offer named: a member removed after the offer could still take the
  block, and with it the right to grant it back to themselves.
- **An acceptance only the server records**, with the owner's single record
  appended once it says yes: the log would not show that the new owner agreed,
  so a server could move a block on its own word.
