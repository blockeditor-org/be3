# Invitations are bearer links that join as editors

An invitation into an organization is a link the inviter sends themselves
(`{server}/#invite/{CODE}`), bound to no email: any account on the server that
opens it may accept it, once, within seven days, and joins as an editor. The
server mails nothing for it. Its row stays after it ends, for the
organization's invite list (guides/blocks/keys.md, "Inviting").

## Why

- Binding an invitation to an email made it useless for an account with no
  email (decisions/an_account_with_no_email_has_an_empty_one.md), and made the
  invitee prove control of an address the inviter only typed.
- A link that works for anyone is a bearer secret, so it must not hand over
  much: an editor reaches only what it is given. An administrator is sealed the
  escrow key and opens everything, so a leaked administrator invitation would
  hand over the organization. Promoting a member who is already the
  administrator's verified contact (`set-role`) is the one way to make one.
- For an editor, the list showing who accepted, and `remove-member`, are enough
  of a check: no confirming step.
- The invitation is read before sign-in (`read-invitation`), so the person
  sees who invited them, and to what, before registering.

## Rejected

- Mailing invitations from the server: it puts the server in the delivery path
  of every relationship and needs real mail delivery
  (plans/deferred/mail_delivery.md); an inviter's own app is the out-of-band
  channel contact links already use.
- Invitations that join as an administrator: see above.
- Showing the invitation only after sign-in: simpler, but sends people into
  registration not knowing who invited them.
