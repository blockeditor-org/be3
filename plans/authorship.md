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
  messages are real; someone holding the old phrase can sign with it, and
  arrival times are what would tell them apart.
- **Timestamps.** A signed time is only what the author claims. A follower's
  message carries when the server took it, stamped by the session owner
  (guides/chat.md, "Arrival time"), which that follower cannot choose; the
  owner's own messages carry none yet (plans/chat.md). The stamp is stored in
  content, so a member who owns a session or commits directly can write any
  arrival; accepted. A member with Edit who removes a message and sends it
  again under its id gives it a later, honest arrival.
- Signatures detect forgery; they don't stop someone with Edit rewriting or
  removing messages (plans/deferred/chat_moderation.md).
