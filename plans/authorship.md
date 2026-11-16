# Authorship

Depends on: none.

Signing what a person writes, so nobody with Edit can post as someone else.
The signing key, its verification and the `sign` and `verify` host calls have
landed (guides/blocks/keys.md, "Signing what a person writes";
decisions/an_ed25519_signing_key_beside_the_identity_key.md), and chat signs
each message and edit and marks one that is not verified, whose key changed,
or that is forged.

## What is left open

- **A whole rollback goes unseen.** Someone with Edit who commits the whole
  header as an earlier state, ranks included, shows nothing: a live change
  never lowers a rank, but a commit is a whole state, and records only detect
  what changes beside them.
- **Taken-back reactions stay.** A reaction taken back is a signed record that
  stays in the header (and in version history), so who once reacted how is
  kept, as for edited bodies.
- **A replaced key still verifies what it signed.** After a contact takes a new
  recovery phrase, its old key shows as changed, not forged, since its earlier
  messages are real. A message stamped as arriving after this device learned of
  the new key shows as forged (guides/blocks/keys.md, Verifying); one with no
  stamp (numbered offline), an edit or reaction (which keep none), or one that
  arrived before this device learned of the change still shows as changed, and
  a member who seals as the lease holder can write any stamp (Timestamps).
- **Timestamps.** A signed time is only what the author claims. A message sent
  in a live session carries when the server took it, stamped by every replica
  from the order (guides/chat.md, "Arrival time"), which its author cannot
  choose; one numbered offline carries none. The stamp is stored in content,
  so a member who seals as the lease holder or commits directly can write any
  arrival; accepted. A member with Edit who removes a message and sends it
  again under its id gives it a later, honest arrival.
- Signatures detect forgery; they don't stop someone with Edit rewriting or
  removing messages (plans/deferred/chat_moderation.md).
