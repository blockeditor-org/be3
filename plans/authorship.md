# Authorship

Depends on: none.

Signing what a person writes, so nobody with Edit can post as someone else. The
signing key and its verification are built (guides/blocks/keys.md, "Signing
what a person writes"; decisions/an_ed25519_signing_key_beside_the_identity_key.md),
and so is the signed log every authored record lives in, with the server's
arrival on each entry (decisions/a_blocks_records_that_only_grow_are_a_signed_log.md).
The host signs each entry an app appends and hands each on with its verdict;
apps never sign or verify anything themselves.
A rollback, a reaction taken back, a deletion with an old key and a forged
arrival are all caught or impossible there.

## What is left open

- **Forks across devices.** Each device keeps the log head it verified and takes
  a new one only with a proof that it extends it, so a server that cuts or
  rewrites a log is caught by every device that saw the head. A server that
  shows two devices two different logs from the start is not: each sees a log
  that is consistent with itself. Catching it needs devices to compare heads,
  carried in what they already exchange (peer messages in a session, a
  commit's `log-through`, contact letters), and a verdict when two heads
  diverge (guides/blocks/keys.md, "Forks").
- **First use of a member's key** is trusted: set aside by the owner
  (plans/deferred/authorship.md).
- **A replaced key and someone holding the old phrase** look alike: an entry
  signed with a key the account replaced, arriving after this device learned of
  the new one, shows as "sent with an old key", not forged, since the account's
  own devices not yet told of the new phrase sign with the old key too. Telling
  them apart would need the time the account took the new phrase, signed in its
  key change, and a grace period for its devices.
