# Chat

A chat block: a group conversation that is an ordinary block, made and shared
like any other. Chat grows forever, so it loads only what is on screen
(a type can load only its first part, guides/blocks/content_types.md "Parts"),
and it is edited by many people at once, so it saves less often than it is
edited (the journal, guides/blocks/client.md). Not implemented
yet.

## What exists

- VISION.md lists chat as planned and not designed.
- **Loading.** A type whose `BlockContent::LOADS` is `Loads::FirstPart` loads
  only its first part, and its editors read the rest by range
  (`LiveContent::read_range`; guides/blocks/apps.md, "Reading a range"). Every
  `Blob` loads that way. A save reuses the parts it never loaded, and
  publishing sends only what changed. Documents save as parts but load whole.
- **Authorship.** `Commit::author` is whatever the client wrote. A session
  names participants by connection (`client-id`), and presence says who a
  connection is. Anyone holding a block's data key can write anything,
  including as someone else. The server records when it received each commit
  (`block_commits.received`).
- **Identity.** A recovery phrase derives an X25519 key pair. The escrow key
  also derives an Ed25519 signing key through HKDF (`EscrowSecret::sign` in
  `crates/be-block-keys/src/lib.rs`). Contacts verify each other's identity
  keys out of band (guides/blocks/keys.md).
- **Access.** Posting needs Edit, which also allows changing others' messages.
  A grant must name a contact, so a chat only ever reaches contacts.
- **Merging.** A `List` merge puts an insert after the sibling it was made
  after (guides/blocks/content_types.md).

## The design

### The block

- One block in the `Streamed` layout:
  - **The header** is the only chunk that changes: a `Document` holding the
    recent messages; edits, deletions and reactions on older messages, keyed by
    message id; and the segment index (each segment's byte range, first and
    last message id, time range, message count and references).
  - **Segments** are the payload: about 64–256 KiB of encoded messages each, one
    chunk per segment, never changed except by a rewrite.
- **A message** is an object of registers, not `Text`: author, claimed time,
  body, the message it replies to, attachments and an empty signature. Its id
  is its `ObjectId`. A draft stays in the editor until sent.
- **Closing a segment** is an operation the session owner sequences once the
  recent messages pass a size: messages a..b become segment k, with its hash.
  Only an owner online, and only for messages already confirmed. Every replica
  encodes the segment's bytes itself and checks them against the hash; the
  operation carrying a part's hash is new (`plans/large_blocks.md`), and chat
  is its first user.
- **Rewriting a segment** folds its pending edits, deletions and reactions in
  as one new chunk.
- **The editor** opens the header and reads segments as the person scrolls
  back.
- **Attachments** are child blocks. Each segment's references are in the
  index, so they never need reading again.

### Sending offline

- A message written offline goes into the document like any edit, and goes out
  through the ordinary three-way merge. Chat's merge puts messages only one
  side added after all of the other side's, keeping their order: a model
  attribute on a `List` (`merge_appends`), not a merge written for chat.
- An offline device never closes segments, so a merge only touches the header.
- **Sending state.** The editor shows a message as sending until it is in the
  published head. In a live session the replica keeps `confirmed` and
  `visible` apart; offline, the editor also needs to know the last published
  head, which it isn't told today.
- A message shows its claimed time. When it lands well after that, it says it
  was sent offline and when it arrived.

### Unread counts

- Where each person has read up to is theirs, not part of the shared document,
  where it would be a read receipt. It needs a home per account, so it matches
  across devices; view state is per view.
- The newest message and the counts come from the header's index, so a sidebar
  of chats reads one small chunk per chat.

### Notifications

A host feature any type can use; chat is the first.

- The sender seals a short notification (chat name, author, preview; about
  1 KiB) under the block's key and calls `notify(block, sealed, mentions)`.
- The server pushes it to members who aren't connected, skipping those who
  muted the chat. The server stores each account's mute settings, and learns
  who was mentioned.
- The receiving device unwraps the key, decrypts, and optionally checks a
  signature: no block load, no type module, no network. The keys must be
  readable from the notification extension (the keychain on iOS). Missing a
  key, it shows a generic "New message".
- Limits to confirm before building: an iOS notification service extension has
  about 30 s and little memory; an Android high-priority message has about
  10–20 s; every web push must show a notification.

### Authorship (after the first version)

- First version: the author is whatever the sender wrote, so anyone with Edit
  can post as someone else. The message keeps an empty signature, and the
  bytes to sign are defined from the start.
- **The signing key.** **(ask)** XEdDSA with the existing identity key, so
  contacts' verified keys and the key-change chain work unchanged; or an
  Ed25519 key derived from the identity secret, added to `ContactCard` and to
  key changes.
- **Host calls.** `sign(bytes)` and `verify(account, bytes, signature)`
  returning verified, unverified key, or bad. Apps never hold the identity
  secret. Both are calls on the app protocol, with no framework types.
- **What a message signs:** chat id, message id, author, claimed time, body
  hash, reply, and the hash of the last message the author saw. That last one
  proves a message was written after it, and shows a gap when one of an
  author's messages is removed.
- **Members who aren't contacts** have keys only from the server: trust them on
  first use, show them differently from contacts, and warn when the key
  changes.
- **Timestamps.** A signed time is only what the author claims. The server
  stamps when it receives a relay, as it already does for commits, so a message
  carries an arrival time no member chose.
- Signatures detect forgery; they don't stop someone with Edit rewriting or
  removing messages. A "post" access level that stops that comes later.

### Deleting

- First version: deleting removes a message from the current state, and it
  stays in version history.
- **Later: a key per message.** Each body (and its edits) is sealed under its
  own key. Message keys are rows on the server, keyed by block and message id
  and wrapped under the block's data key. The author's own connection creates
  the row, and the server records which account did. Only that account, or
  someone with Manage, may delete it; deleting it makes every copy in history
  unreadable without touching a commit. Someone else removing a message only
  changes the document, so history can bring it back.
- A segment's message keys come in one batched call beside its chunk. In a live
  session the sender sends the key with the message. Offline, the row waits in
  the outbox with the commit.
- Rewriting history instead would change every later commit's id, since commits
  name their parents inside their sealed objects.

## Order of work

1. The chat type and editor: the block, `merge_appends`, sending state, unread
   counts.
2. Notifications.
3. Authorship: signing, verifying, server arrival times.
4. Message keys for deleting.
5. A "post" access level.

## Not planned here

- Direct messages: a chat is made and shared like any block.
- Search: a feature of the whole app, which has to cope with large blocks.
- Freezing chat's format: once it has users.
