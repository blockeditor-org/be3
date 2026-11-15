# Share links

Depends on: plans/signed_access.md; plans/hosted_blocks.md step 3 for step 3
only.

"Anyone with the link": a link that grants any level up to Edit on a block to
whoever opens it, with no grant naming them first. It is passed outside the host, so it
is the out-of-band start that every sharing relationship needs
(decisions/share_links_are_an_out_of_band_start.md). The server must
not be able to use a link it relays or stores
(decisions/the_server_never_vouches_for_an_identity.md).

## Shape

- A link is `{server}/#share/{SECRET}`. The secret derives a link key pair
  and, for View and above, a key that opens the block's scope key, sealed with
  it as pending grants are (`Peer::offer`). The secret part also carries
  the hash of the block's log root, which the opener pins
  (plans/signed_access.md, "Root"). The server never sees what follows `#`.
- Making one stores a **link record** in the block's access log
  (plans/signed_access.md): block, level, the link's public key, signed by a
  member with Manage.
- Opening one: the account signs a grant to its own signing key with the link
  key and adds it to the log. A device checks it through the link record, so a
  grant from a link is as checkable as one from a member, and the account is
  listed in the access list as joined through that link. The server's rule
  that a grant names a contact of its granter (guides/blocks/server.md,
  "Organizations and access") gives way to the link record here.
- Revoking the link is a signed revocation of its record; every grant made
  through it falls with it, and the block rotates as for any lost access.
- **Who may open it** is chosen in the share dialog, within what the
  organization's policy allows (a setting held in its settings, which the
  server enforces): a member of the block's organization (the default), anyone
  with an account on the server, or anyone at all. Opening anonymously makes
  an account on the server on the spot (not a local account, which has no
  server; plans/local_accounts.md), kept on that device alone with no recovery
  phrase shown, which signs its grant like any other. In the personal organization
  every account is a member, so the first two are the same.
- Opening a link makes nobody a contact; a contact link or an invitation does
  that. So a share link is not a `link-kind` of the contact link family
  (`contacts.wit`, guides/blocks/keys.md "Contacts"), and never passes through
  `accept-link`.
- A Join link (plans/hosted_blocks.md) carries no scope key: it grants Join,
  and a host admits the account through its grant.

## Order of work

1. Link records, opening and revoking, for View and Edit. Done: a test where a
   link grants View to an account no one granted; the server, holding the
   stored link record and every relayed message, opens nothing.
2. The share dialog's "Anyone with the link" row, choosing the level (up to
   Edit), who may open it and revoking, and the organization policy. Done: a
   painting; a test where a link that admits only members refuses another
   account, and one where an anonymous opener edits.
3. Join links, once plans/hosted_blocks.md step 3 lands.
