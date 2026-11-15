# Authorship

Depends on: none.

Signing what a person writes, so nobody with Edit can post as someone else.
The signing key, its verification and the `sign` and `verify` host calls have
landed (guides/blocks/keys.md, "Signing what a person writes";
decisions/an_ed25519_signing_key_beside_the_identity_key.md), and chat signs
each message and edit and marks one that is not verified, whose key changed,
or that is forged.

## What is left open

- **A recent message can be dropped unnoticed.** Removing a message from the
  recent series leaves no record a signature covers; only the `seen` hashes of
  its author's later messages show the gap (below).
- **A chain restarts after a forged overwrite.** An author's next edit or
  reaction names only their own latest record as `previous`, so once a record
  of theirs is overwritten by someone else's (which shows as forged), their
  records from before it are never superseded and can be put back later
  without showing as forged.
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
