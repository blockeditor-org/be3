# Chat

Depends on: plans/large_blocks.md (`merge3` over parts), for step 1d only;
steps 1a to 1c start at once. Owner-only settings, for step 1c, have landed
(guides/settings.md, Owner only). plans/notifications.md for step 2.

A chat block: a group conversation that is an ordinary block, made and shared
like any other. Chat grows forever, so it loads only what is on screen
(a type can load only its first part, guides/blocks/content_types.md "Parts"),
and it is edited by many people at once, so it saves less often than it is
edited (the journal, guides/blocks/client.md). Step 1a has landed.

## What exists

- **The type and editor (1a).** `crates/be-type-chat` and `crates/be-app-chat`.
  `ChatContent` is written by hand, wrapping the header `Document<ChatHeader>`
  (its only part, `Loads::FirstPart`) and forwarding `LiveEdit`, `Merge` and
  `Undo` to it. The header holds the messages (a `merge_appends` list),
  `edits`, `deletions` and `reactions` keyed by message id, and an empty
  `segments` index. Each message stores the hash of the last message its
  author saw (`seen`) and an empty signature; `Message::signed_bytes`,
  `MessageEdit::signed_bytes`, `Deletion::signed_bytes` and
  `ReactionKey::signed_bytes` define what a signature will cover
  (`src/signed.rs`), and a message's hash is SHA-256 over its signed bytes.
  Deleting removes the message with its edit and reactions and records the
  `Deletion`. The editor sends, replies, edits and deletes its own messages and
  toggles reactions (Lucide icon names); members other than you show as
  "Member" and their account's first digits, since no host service names
  members to an app yet. Attachments are in the model but the editor cannot
  add or show them yet.
- **Loading.** A type whose `BlockContent::LOADS` is `Loads::FirstPart` loads
  only its first part, and its editors read the rest by range
  (`LiveContent::read_range`; guides/blocks/apps.md, "Reading a range"). Every
  `Blob` loads that way, a content type written by hand rather than derived. A
  save reuses the parts it never loaded, and publishing sends only what
  changed. Documents save as parts but load whole.
- **References per part.** A part's node lists the blocks its own bytes
  reference (`Part::Node { references }`), so a part reused unread keeps them.
- **Authorship.** `Commit::author` is whatever the client wrote; anyone holding
  a block's data key can write anything, including as someone else
  (plans/authorship.md).
- **Access.** Posting needs Edit, which also allows changing others' messages.
  A grant must name a contact, so a chat only ever reaches contacts.
- **Merging.** A `List` merge puts an insert after the sibling it was made
  after (guides/blocks/content_types.md). Publishing is a compare-and-swap on
  the head: a device whose publish is rejected merges again on the new head,
  so a merge never has to give the same result from both sides.

## The design

### The block

- **The content type is written by hand**, as `Blob` is: it wraps a header
  `Document` and the segment parts, and forwards `LiveEdit`, `Merge` and `Undo`
  to the header. It is not a derived `Document`, since a `Document` loads
  whole.
- **The header** is the first part and the only one that changes in steps 1a
  to 1c: the recent messages; every edit, deletion and reaction, on recent and
  older messages alike, kept apart from the messages and keyed by message id,
  so closing a segment never drops one; and the segment index (each segment's first
  and last message id, time range and message count).
- **Segments** (step 1d) are the other parts: about 64–256 KiB of encoded
  messages each, one part per segment, never changed except by a rewrite. Each
  segment's references are its node's references, not a list in the header.
- **A message** is an object of registers, not `Text`: author, claimed time,
  body, the message it replies to, attachments and an empty signature. Its id
  is its `ObjectId`. 1a also defines a message's hash (over its signed bytes)
  and stores in each message the hash of the last message its author saw, and
  the bytes a signature will cover, for messages, edits, deletions and
  reactions alike, are defined from the start (plans/authorship.md), so
  signing later changes no format. A draft
  stays in the editor until sent.
- **Closing a segment** is an operation the session owner sequences once the
  recent messages pass a size: messages a..b become segment k, carried by its
  hash (`put-part`, guides/type_modules.md, "Parts named by hash").
  Only an owner online, and only for messages already confirmed. Every replica
  encodes the segment's bytes itself and checks them against the hash; on a
  mismatch (another build encoding differently) it reads the part with
  `read-part` instead. Missing: `put-part` is reachable only from an editor
  (`FromEditor::PutPart`), so the owner closing a segment with no editor
  attached needs a type-side hook first (a `LiveEdit` way to ask for a part
  and sequence the operation that names it once it is put).
- **Rewriting a segment** folds its pending edits, deletions and reactions in
  as one new part.
- **The editor** opens the header and reads segments as the person scrolls
  back.
- **Attachments** are child blocks.

### Sending offline

- A message written offline goes into the document like any edit, and goes out
  through the ordinary three-way merge. Chat's merge puts the messages only one
  side added after all of the other side's, ours first, keeping their order: a
  model attribute on a `List` (`merge_appends`), not a merge written for chat.
- An offline device never closes segments. The online side may have closed
  one since the base, moving a message an offline edit or reaction names into
  a segment. Edits, deletions and reactions are kept by message id whether the
  message is recent or not, so the ordinary merge keeps that edit by key and
  the next rewrite folds it into the segment. Nothing is lost and nothing
  conflicts.
- **Sending state.** A message shows as sending until it is confirmed: in a
  live session, sequenced by the owner (the replica keeps `confirmed` and
  `visible` apart); offline, published. For the offline case the editor is
  told the last published head through a new field, `published-head`, beside
  the head in the content frame (guides/blocks/apps.md).
- **Arrival time.** The server stamps each relayed operation with when it
  received it, in the relay envelope (commits already have
  `block_commits.received`), and replicas store that time in the message. A
  message shows its claimed time; once stamped, if it arrived well after
  that, it says it was sent offline and when it arrived. A dishonest session
  owner could alter stored content, which plans/authorship.md accepts.

### Unread counts

- Where each person has read up to is theirs, not part of the shared document,
  where it would be a read receipt. It is kept in an owner-only settings kind
  of chat's own (`KindVisibility::Owner`, guides/settings.md), keyed by chat id, so it matches
  across devices; ordinary user settings are escrowed, and administrators
  could read them.
- The newest message and the counts come from the header, so a sidebar of
  chats reads one small part per chat.

### Deleting

- Deleting removes a message from the current state, and it stays in version
  history. Keys per message that make a deletion final are deferred
  (plans/deferred/chat_moderation.md).

## Order of work

1. **The chat type and editor.**
   - **1b.** Sending state and `published-head`. Done: a host test sends a
     message while the server is unreachable and the painting shows it as
     sending until the drain publishes it.
   - **1c.** Unread counts in an owner-only kind. Done: reading on one device clears
     the count on another; a painting of the count.
   - **1d.** Segments, closing and rewriting them. Done: a chat of thousands of
     messages opens reading only the header; scrolling back reads segments by
     range; an offline reaction to a message moved into a segment survives the
     merge; a forced hash mismatch reads the part instead.
2. **Notifications** for chat, on plans/notifications.md. Done: a member who is
   not connected gets a notification naming the chat and author, and none for
   a muted chat.

## Not planned here

- Direct messages: a chat is made and shared like any block.
- Search: plans/search.md.
- Signing messages: plans/authorship.md.
- Message keys for deleting and a "post" access level:
  plans/deferred/chat_moderation.md.
- Freezing chat's format: once it has users.
