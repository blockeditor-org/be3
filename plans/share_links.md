# Share links

Depends on: none (plans/hosted_blocks.md step 3, which step 3 here needs, has landed).

"Anyone with the link": a link that grants any level up to Edit on a block to
whoever opens it, with no grant naming them first. It is passed outside the
host, so it is the out-of-band start that every sharing relationship needs
(decisions/share_links_are_an_out_of_band_start.md). The server must not be
able to use a link it relays or stores
(decisions/the_server_never_vouches_for_an_identity.md).

## Shape

- A link is `{server}/#share/{SECRET}`. The server never sees what follows
  `#`. The secret derives a **lookup id**, which the opener sends to find the
  link (as `InviteCode::lookup` names an invitation), a link key pair, and,
  for View and above, a key that opens the block's scope key, sealed with it
  as pending grants are (`Peer::offer`). The secret also carries the hashes of
  the block's log root and of its organization's root, which the opener pins
  (guides/blocks/keys.md, "Access logs"), so a guest can check records an
  administrator signed.
- Making one stores a **link record** in the block's access log
  (guides/blocks/keys.md, "Access logs"): level, who may open it, the lookup id and the
  link's public key, signed by a member with Manage.
- Opening one: the account signs a grant to its own keys with the link key
  and adds it to the log. A device checks it through the link record, so a
  grant from a link is as checkable as one from a member, and the account is
  listed in the access list as joined through that link. The server's rule
  that a grant names a contact of its granter (guides/blocks/server.md,
  "Organizations and access") gives way to the link record here.
- **Keys after a rotation.** The scope key in the link opens the epochs
  current when the link was made. Later epochs reach openers through key
  work's `unsealed`, as for any grantee: their grant names their keys, so any
  editor's `tend` seals to them (guides/blocks/keys.md,
  "Trust"). An opener after a rotation therefore waits for an
  editor to come online. The maker keeps nothing.
- Revoking the link is a signed revocation of its record; every grant made
  through it falls with it, and the block rotates as for any lost access.
- **Who may open it** is chosen in the share dialog, within what the
  organization's policy allows (a setting held in its settings, which the
  server enforces): a member of the block's organization (the default), or
  any account on the server. In the personal organization every account is a
  member, so the two are the same. Anyone at all, with an account made on the
  spot, is set aside (plans/deferred/anonymous_share_links.md).
- **Organization membership.** Opening a link does not join the organization:
  an invitation stays the only way in (guides/blocks/keys.md, "Inviting"). An
  opener from outside a company organization becomes a **guest** of it: a
  member row that holds only grants made through links, gets no escrow key,
  sees no organization roots or members, and is removed when its last such
  grant falls. A guest may not create blocks, grant access or make links
  there, so nothing it made outlives it. Its be-host shows the organization
  only through what it was shared, in Shared with me: no workspace, roots or
  files pane of its own there.
- Opening a link makes nobody a contact; a contact link or an invitation does
  that. So a share link is not a `link-kind` of the contact link family
  (`contacts.wit`, guides/blocks/keys.md "Contacts"), and never passes through
  `accept-link`.
- A Join link (plans/hosted_blocks.md) carries no scope key: it grants Join,
  and a host admits the account through its grant. On a hosted type a link
  grants Join by default; a higher level is an explicit choice in the dialog.

## Order of work

1. Link records, opening and revoking, for View and Edit, guests, and keys
   after a rotation. Done: a test where a link grants View to an account no
   one granted; the server, holding the stored link record and every relayed
   message, opens nothing; one where a link opened after a rotation unlocks
   once an editor tends.
2. The share dialog's "Anyone with the link" row, choosing the level (up to
   Edit), who may open it and revoking, and the organization policy. Done: a
   painting; a test where a link that admits only members refuses another
   account, and one where an account from another organization opens one as
   a guest.
3. Join links, once plans/hosted_blocks.md step 3 lands.
4. **Guides.** guides/blocks/keys.md (links, guests) and
   guides/blocks/server.md (guests, the link policy).
