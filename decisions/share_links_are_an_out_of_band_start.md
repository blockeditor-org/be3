# A share link is an out-of-band start

A share link (plans/share_links.md) passed outside the app is an out-of-band
start, as a contact link is, so "no in-app reach to strangers" holds for it.
That holds whoever the link admits: members of the block's organization or
any account on the server (anyone at all, with no account, is deferred:
plans/deferred/anonymous_share_links.md).

- Opening a link makes nobody a contact. It grants access to one block and
  nothing else; becoming contacts still takes a contact link or an
  invitation. An opener from outside the organization becomes a guest, which
  may not create blocks, grant access or make links there.
- A grant checked in the block's access log stands in for a contact check
  when a device seals a key (guides/blocks/keys.md,
  "Trust"): the grant names the grantee's keys as a member with the
  right level verified them, so a sealer needs no contact row of its own, and
  the grantee accepts a key from a sender a checked record names.
- A Join member (plans/hosted_blocks.md) may message the block's host over the
  channel the server relays, and the app may show which account hosts. The link
  that admitted them started the relationship, and the channel reaches only
  that block's host.

## Why

What the principle guards against is someone reaching a person who never
handed them a way in. A link is a way in that a member chose to make and pass
on. Whoever opens it was given it outside the app, as a contact link is. A game
shared with "anyone with the link" is useless if its players can't talk to its
host, or can't see who is hosting.

## Rejected

- **Only contacts may open a link:** "anyone with the link" would need a
  contact exchange first, which is the step a link exists to skip.
- **Opening a link makes the opener and the link's maker contacts:** someone
  who opens a game world's link has not asked to be anyone's contact, and a
  contact is reachable for every block, not one.
