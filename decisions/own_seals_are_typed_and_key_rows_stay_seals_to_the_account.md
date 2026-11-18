# Own seals are typed, and key rows stay seals to the account

What an account sends itself through the server in a letter (its record of a
contact, what it pinned for an organization) is sealed with a key derived from
its identity secret under a context per kind (`RecoverySecret::seal_own`,
`be_block_keys::Own`), so one kind never opens as another. A key row the account
seals for itself is sealed to its own account with `seal_for`, as a row to a
contact is.

## Why

A letter is no key row: the server holds it whole, and sealing it to the public
key would let the server make one up. A key row to oneself is a row like any
other: the server checks that a new phrase reseals every row sealed for the
account, and every row opens by one path, by its sender's key. Typing the own
seals closes a letter of one kind replayed as another.

## Rejected

- One seal to oneself for both (the crypto audit's "seal to oneself two ways"):
  rows sealed with `seal_own` would need a second opening path and could no
  longer be told apart from a contact's seal by the server's checks.
- Keeping one context for every own seal: only some callers checked what they
  opened.
