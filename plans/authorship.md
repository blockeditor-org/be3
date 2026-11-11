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
- **An older edit can be replayed.** An edit is bound to the hash of its message
  (so it verifies on no other message), but not to the edit it replaces: the
  content keeps only the latest edit, so a verifier cannot tell that an older
  signed edit by the same author is not the newest, and anyone with Edit can put
  one back. Fixing it needs the edits kept as a chain (each signing the previous
  one's hash, the message noting the newest), or the arrival time below.
- **First use of a member's key is trusted.** A member who is not a contact has
  the key the server named the first time, so a dishonest server can name one it
  holds then. A contact's key is never taken from the server.
- **A replaced key still verifies what it signed.** After a contact takes a new
  recovery phrase, its old key shows as changed, not forged, since its earlier
  messages are real; someone holding the old phrase can sign with it, and
  arrival times are what would tell them apart.
- **A server or participant can drop or reorder.** A deleted or withheld message
  shows only as a gap in the `seen` hashes of its author's later messages;
  nothing checks them yet.
- **`verify` costs a server round trip** for each forged row; cache or throttle.
- **Timestamps.** A signed time is only what the author claims. The server is
  to stamp each relayed operation with when it received it (plans/chat.md,
  "Arrival time"), so a message carries an arrival time no member chose. The
  stamp is stored in content, which a dishonest session owner could alter;
  accepted.
- Signatures detect forgery; they don't stop someone with Edit rewriting or
  removing messages (plans/deferred/chat_moderation.md).

## Order of work

1. **Sign the rest.** Chain each edit to the one it replaces, sign deletions and reactions,
   including a reaction taken back as a record with no value, and verify them
   in the editor. Done: a replayed edit, a forged deletion and a reaction taken
   back by someone else each show as forged.
2. **Arrival times** in chat, once the server stamps relayed operations. Done:
   a message sent offline shows when it arrived; a painting.
