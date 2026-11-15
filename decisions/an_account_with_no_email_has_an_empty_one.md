# An account with no email has the empty string as its email

An account made through an OpenID Connect provider that gives no verified email
has no email (`accounts.email` is NULL). Everywhere outside the database, in
`Profile`, the protocol's `email` fields and the portal's pages, its email is
the empty string, as a local profile's already was, and clients show only the
name for it.

An account needs no email at all: one whose identities are only providers or
passkeys is allowed, and those are its recovery. A provider's verified email
links to an existing account only after a code sent to that email is entered
in the same browser, and an email is held by one account at most.

## Why

- Local profiles already had no email and sent the empty string end to end, so
  every client (be-host's contact cards, members list and share dialog, "Connect
  with NAME (EMAIL)?") already shows only the name for one. Accounts with no
  email reuse that one form.
- The database keeps NULL so the email's uniqueness covers only real emails.

- Linking a provider to the account that holds its email without a code would
  let anyone who controls an account at a provider the operator trusts for
  email, but not the mailbox, take over the be3 account.

## Rejected

- `option<string>` in the protocol and `Option<String>` in `Profile`: a second
  way to say "no email" beside the one local profiles use, changing every
  client for no behaviour it doesn't already have.
