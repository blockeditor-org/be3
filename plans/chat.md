# Chat

Depends on: none; merging by parts (guides/type_modules.md, "Merging by
parts"), which step 1d uses, has landed. Step 2 needs plans/notifications.md step 1
(a device that shows a push); the server's side and the `notify` host call
it uses have landed.

A chat block: a group conversation that is an ordinary block, made and shared
like any other. Chat grows forever, so it loads only what is on screen
(a type can load only its first part, guides/blocks/content_types.md "Parts"),
and it is edited by many people at once, so it saves less often than it is
edited (the journal, guides/blocks/client.md). Steps 1a and 1c have landed.

## What exists

- **The type and editor (1a).** `crates/be-type-chat` and `crates/be-app-chat`.
  `ChatContent` is written by hand, wrapping the header `Document<ChatHeader>`
  (its only part, `Loads::FirstPart`) and forwarding `LiveEdit`, `Merge` and
  `Undo` to it. The header holds the messages (a `merge_appends` list),
  `edits`, `deletions` and `reactions` (`LatestMap`s) keyed by message id, and
  an empty `segments` index. Sending and deleting are not undoable; an edit by
  anyone but the message's author is ignored. Each message stores the hash of the last message its
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
- **Unread counts (1c).** Where the account read each chat up to is theirs,
  never in the chat, where it would be a read receipt: the owner-only settings
  kind `ChatReads` (guides/settings.md, Owner only; ordinary settings are
  escrowed, and administrators could read them), one place per device, so a
  device behind or offline never takes another's reading back. The furthest
  place counts, by message order: a deleted message's `Deletion` names the one
  before it. Each place is a key of its own (`chat/device/message`), and a
  device moving on drops only places it has seen that are not further than
  its new one, so a move made elsewhere meanwhile survives.
  `ChatContent::unread` counts others' messages after it and after my own
  newest message (answering is reading), reading only their authors. The editor reads the chat while it is shown, its view has the
  keyboard and it is scrolled to its newest message; otherwise it shows "N new
  messages" above the message box, which scrolls to the end. No sidebar or
  block row shows the count yet.
- **Member names.** Apps have no way to learn a member's name: the `contacts`
  watch is the shell's alone. Step 1b or a later one needs a host service that
  names the accounts in a block's members (contacts by their card, others by
  what the server says, marked unverified), with no framework types.
- **Loading.** A type whose `BlockContent::LOADS` is `Loads::FirstPart` loads
  only its first part, and its editors read the rest by range
  (`LiveContent::read_range`; guides/blocks/apps.md, "Reading a range"). Every
  `Blob` and `ChatContent` load that way, content types written by hand rather
  than derived. A
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
  after, except in a `merge_appends` list such as chat's messages, which keeps
  the published side as it is and puts what only the merging device added
  after it (guides/blocks/content_types.md). Edits, deletions and reactions are
  `LatestMap`s, so the later write of a key wins and none conflicts. Publishing is a compare-and-swap on
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
  through the ordinary three-way merge. Chat's merge is published first: the
  messages already on the published head (theirs, in every merge a replica or
  `be sync` makes) keep their place, and the merging device's own new messages
  go after all of them, in their order: a model attribute on a `List`
  (`merge_appends`), not a merge written for chat. In a live session the owner
  sequences each send, an insert at the end, so a message lands after those
  sequenced before it, the same order.
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

- The newest message and the counts come from the header, so a sidebar of
  chats (not built) reads one small part per chat.
- With segments (1d), `messages()` holds only the recent ones, and a place it
  does not hold reads as "not received yet", so all read. 1d has to find a
  place in an older segment by the segment index instead.

### Deleting

- Deleting removes a message from the current state, and it stays in version
  history. Keys per message that make a deletion final are deferred
  (plans/deferred/chat_moderation.md).

## Order of work

1. **The chat type and editor.**
   - **1b.** Sending state and `published-head`. Done: a host test sends a
     message while the server is unreachable and the painting shows it as
     sending until the drain publishes it.
   - **1d.** Segments, closing and rewriting them. Done: a chat of thousands of
     messages opens reading only the header; scrolling back reads segments by
     range; an offline reaction to a message moved into a segment survives the
     merge; a forced hash mismatch reads the part instead.
2. **Notifications** for chat: sending a message calls `notify` with its text
   and the accounts it mentions (guides/apps/host_services.md), once the
   message is confirmed, since `notify` is not ordered with the save that
   carries it. Done: a member who is
   not connected gets a notification naming the chat and author, and none for
   a muted chat.

## Not planned here

- Direct messages: a chat is made and shared like any block.
- Search: plans/search.md.
- Signing messages: plans/authorship.md.
- Message keys for deleting and a "post" access level:
  plans/deferred/chat_moderation.md.
- Freezing chat's format: once it has users.
