# Authorship

Depends on: plans/chat.md, "Arrival time" (the server stamping relayed
operations), for step 2.

Signing what a person writes, so nobody with Edit can post as someone else.
The signing key, its verification and the `sign` and `verify` host calls have
landed (guides/blocks/keys.md, "Signing what a person writes";
decisions/an_ed25519_signing_key_beside_the_identity_key.md), and chat signs
each message and edit and marks one that is not verified, whose key changed,
or that is forged.

## What is left open

- **Edits of messages in closed segments are not chained**: the editor cannot
  hash a closed message from the header alone, so their edits name no previous
  one and a replayed one is not caught.
- **A replaced key still verifies what it signed.** After a contact takes a new
  recovery phrase, its old key shows as changed, not forged, since its earlier
  messages are real; someone holding the old phrase can sign with it, and
  arrival times are what would tell them apart.
- **Timestamps.** A signed time is only what the author claims. The server is
  to stamp each relayed operation with when it received it (plans/chat.md,
  "Arrival time"), so a message carries an arrival time no member chose. The
  stamp is stored in content, which a dishonest session owner could alter;
  accepted.
- Signatures detect forgery; they don't stop someone with Edit rewriting or
  removing messages (plans/deferred/chat_moderation.md).

## Order of work

1. **Arrival times** in chat, once the server stamps relayed operations. Done:
   a message sent offline shows when it arrived; a painting.
