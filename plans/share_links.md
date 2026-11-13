# Share links

Depends on: plans/signed_access.md.

"Anyone with the link": a link that grants any level on a block to whoever
opens it, with no grant naming them first. It is passed outside the host, so it
is the out-of-band start that every sharing relationship needs. The server must
not be able to use a link it relays or stores
(decisions/the_server_never_vouches_for_an_identity.md).

## Shape

- A link is `{server}/#share/{SECRET}`. The secret derives a link key pair
  and, for View and above, a key that opens the block's scope key, sealed with
  it as pending grants are (`Peer::offer`). The server never sees what follows
  `#`.
- Making one stores a **link record** in the block's access log
  (plans/signed_access.md): block, level, the link's public key, signed by a
  member with Manage.
- Opening one: the account signs a grant to its own signing key with the link
  key and adds it to the log. A device checks it through the link record, so a
  grant from a link is as checkable as one from a member, and the account is
  listed in the access list as joined through that link.
- Revoking the link is a signed revocation of its record; every grant made
  through it falls with it, and the block rotates as for any lost access.
- A Join link (plans/hosted_blocks.md) carries no scope key: it grants Join,
  and a host admits the account through its grant.

## Order of work

1. Link records, opening and revoking, for View and Edit. Done: a test where a
   link grants View to an account no one granted; the server, holding the
   stored link record and every relayed message, opens nothing.
2. The share dialog's "Anyone with the link" row, choosing the level and
   revoking. Done: a painting.
3. Join links, once plans/hosted_blocks.md step 3 lands.

## Open questions

- **Q4. Manage by link.** May a link grant Manage, behind a warning, or stop at
  Edit?
- **Q5. Who may open one.** Must the person sign in to an account of the
  block's organization, may a company organization's link admit someone
  outside it (a guest member made on opening), or may someone with no account
  join at all (Join only, as an account made on the spot and kept on that
  device)?
- **Q6. Contacts.** A contact link and an invitation make the two accounts
  contacts. Should a share link not, so sharing a game link with a hundred
  people adds nobody to anyone's contacts?
