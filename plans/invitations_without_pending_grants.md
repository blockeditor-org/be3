# Invitations without pending grants

Depends on: none.

Two kinds of link each grant access to one block:

- A contact link that carries a `PendingGrant` (`LinkKind::Contact(Some(grant))`;
  be-block-client's `contacts.rs`, be-server's `contacts.rs`). This is the one grant the server
  applies first, before the maker's device signs it in `finish_links` (guides/blocks/keys.md).
- A share link (`share_links.rs` on both sides), whose opener signs their own grant with the
  link's key (`Record::Opened`).

The share dialog offers both (be-sdk-shell's `workspace.rs`: `ShareAction::InviteNew` calls
`contact_link(block, access)`, `ShareAction::MakeLink` calls `make_share_link`). The share link
already does the job without the server applying anything, so the pending grant is a second,
weaker way to the same result.

## Steps

1. **Single-use share links.** A share link may admit one opener (a new `link-admits` value):
   the server refuses a second open of it, as it refuses an expired link.
2. **"Invite someone new" makes two links in one.** It makes a plain contact link and a
   single-use share link for the block at the chosen access, and hands them out as one
   invitation, so the person still sends one link. Opening it makes the contact and opens the
   share link, as each does alone.
3. **Remove the pending grant.** `PendingGrant`, `LinkKind::Contact`'s grant, its server
   columns, the server-first grant path and the grant-signing half of `finish_links` go.
   guides/blocks/keys.md loses its exception.

decisions/share_links_are_an_out_of_band_start.md stands: opening the share link still makes
nobody a contact; the contact link does that.

## The test

- An invitation opened by a new person makes them a contact and gives them the access chosen,
  with no grant applied by the server.
- A second person opening the same invitation becomes nobody's contact through the share half
  and gains no access.
