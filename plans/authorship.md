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

- **Deletions and reactions are not signed.** Their `Signature` stays empty
  and the editor neither signs nor checks them. Taking a reaction back removes
  its key (a `LatestMap` entry with no value), which carries no signature, so
  anyone with Edit can remove another's reaction unnoticed.
- **An older edit can be replayed.** An edit's signed bytes name the message
  but not its hash nor the edit it replaces, so anyone with Edit can put back
  an older signed edit by the same author, and it verifies.
- **Timestamps.** A signed time is only what the author claims. The server is
  to stamp each relayed operation with when it received it (plans/chat.md,
  "Arrival time"), so a message carries an arrival time no member chose. The
  stamp is stored in content, which a dishonest session owner could alter;
  accepted.
- Signatures detect forgery; they don't stop someone with Edit rewriting or
  removing messages (plans/deferred/chat_moderation.md).

## Order of work

1. **Sign the rest.** Bind each edit to the message's hash and a sequence
   number (or the previous edit's hash), sign deletions and reactions,
   including a reaction taken back as a record with no value, and verify them
   in the editor. Done: a replayed edit, a forged deletion and a reaction taken
   back by someone else each show as forged.
2. **Arrival times** in chat, once the server stamps relayed operations. Done:
   a message sent offline shows when it arrived; a painting.
