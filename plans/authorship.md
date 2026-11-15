# Authorship

Depends on: none.

Signing what a person writes, so nobody with Edit can post as someone else.
The signing key, its verification and the `sign` and `verify` host calls have
landed (guides/blocks/keys.md, "Signing what a person writes";
decisions/an_ed25519_signing_key_beside_the_identity_key.md), and chat signs
each message and edit and marks one that is not verified, whose key changed,
or that is forged.

## What is left open

- **Bounding the record chains.** Each edit or reaction record replaced
  leaves its removed key and stamp in the header's `LatestMap` for good
  (guides/chat.md, "Records chain"), and every replica loads the header. Only
  a removed key counted past its slot's head carries meaning; the others could
  go. That needs a `LatestMap` that forgets keys (a close op that drops a
  closed message's keys outright, or tombstone collection, with a rule for
  merging a replica that never saw it); then a close can move a closed
  message's records into its segment's part, as trustworthy as the part is
  (guides/chat.md, "Segments"), leaving the header recent slots' heads.
- **A whole rollback goes unseen.** Someone with Edit who rewrites the whole
  header to an earlier state, cut keys included, shows nothing: records only
  detect what changes beside them.
- **Taken-back reactions stay.** A reaction taken back is a signed record that
  stays in the header (and in version history), so who once reacted how is
  kept, as for edited bodies.
- **A replaced key still verifies what it signed.** After a contact takes a new
  recovery phrase, its old key shows as changed, not forged, since its earlier
  messages are real; someone holding the old phrase can sign with it, and
  arrival times are what would tell them apart.
- **Timestamps.** A signed time is only what the author claims. A message sent
  in a live session carries when the server took it, stamped by every replica
  from the order (guides/chat.md, "Arrival time"), which its author cannot
  choose; one numbered offline carries none. The stamp is stored in content,
  so a member who seals as the lease holder or commits directly can write any
  arrival; accepted. A member with Edit who removes a message and sends it
  again under its id gives it a later, honest arrival.
- Signatures detect forgery; they don't stop someone with Edit rewriting or
  removing messages (plans/deferred/chat_moderation.md).
