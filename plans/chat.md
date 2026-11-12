# Chat

Depends on: none; merging by parts (guides/type_modules.md, "Merging by
parts"), which step 1d uses, has landed. Step 2 has landed up to the push
leaving the server; a device showing it, naming the chat and author, is
plans/notifications.md step 1, built except a Chromium test of the whole page.

A chat block: a group conversation that is an ordinary block, made and shared
like any other. Chat grows forever, so it loads only what is on screen
(a type can load only its first part, guides/blocks/content_types.md "Parts"),
and it is edited by many people at once, so it saves less often than it is
edited (the journal, guides/blocks/client.md). Steps 1a to 1c and 2 have landed.

## What exists

- **The type and editor (1a).** `crates/be-type-chat` and `crates/be-app-chat`.
  `ChatContent` is written by hand, wrapping the header `Document<ChatHeader>`
  (its only part, `Loads::FirstPart`) and forwarding `LiveEdit`, `Merge` and
  `Undo` to it. The header holds the messages (a `merge_appends` list),
  `edits`, `deletions` and `reactions` (`LatestMap`s) keyed by message id, and
  an empty `segments` index. Sending and deleting are not undoable; an edit by
  anyone but the message's author is ignored. Each message stores the hash of the last message its
  author saw (`seen`) and its author's signature; `Message::signed_bytes`,
  `MessageEdit::signed_bytes`, `Deletion::signed_bytes` and
  `ReactionKey::signed_bytes` define what a signature covers
  (`src/signed.rs`), and a message's hash is SHA-256 over its signed bytes.
  Deleting removes the message with its edit and reactions and records the
  `Deletion`. The editor sends, replies, edits and deletes its own messages and
  toggles reactions (Lucide icon names); members other than you show as
  "Member" and their account's first digits, since no host service names
  members to an app yet. Attachments are in the model but the editor cannot
  add or show them yet.
- **Sending state (1b).** A message shows "Sending" while the server lacks
  the edit that sent it, and "Not sent" once the server refused the commit
  holding it (`ContentProjection::lacking`, guides/blocks/apps.md "Sending"):
  the replica keeps those edits until the server holds them, across editors
  and replicas, so switching chats and back shows them still sending. For an
  owner alone that is every send for `APPEND_INTERVAL` (250 ms) and a round
  trip, even online. A message the server refused shows "Not sent", with Send
  again and Discard. A host or type module restart keeps a commit still in the outbox
  as sending, but loses edits not yet in a commit (a journal entry the server
  lacks); a content-level answer (what the local head adds over the server's)
  would close that, which is the user's call. `drive` has no way to cut the
  server, so there is no be-host-tests test of this: be-host-stack tests and
  chat's editor tests stand in. A `drive` command that cuts the server would let a host test cover it.
- **Notifications (2).** Each message sent from this device tells the chat's
  other members once the server holds it, exactly once, tied to the edit rather
  than to an editor or replica (guides/type_modules.md, "Announcements"):
  `LiveEdit::announcements` names, as the message is sent, its body and the
  author of the message it replies to as a mention, keyed by the message; the
  host keeps that beside the journal entry or outbox commit holding the send and
  sends it once the server holds that. So it goes once however many editors are
  open, or none (a chat closed while its outbox drains), after a restart with
  the message unsent, and not again after a restart once told; a message the
  server refused, or one discarded, tells nobody. A message tells its body as
  sent; editing it tells nobody. A burst drained from the outbox is one notification,
  "N new messages". The host cuts the text to `MAX_NOTIFICATION_TEXT`. Mentions
  are the replied-to author, since apps cannot yet name a member otherwise; a
  member whose mode for the chat is "mentions only" is told for those. The
  server tells only members with no connection open, none whose mode is "off",
  and not the sender's account (plans/notifications.md). Tests: be-host-stack's
  `a_chat_message_*` (members away, connected, off and mentions only; a closed
  chat draining; a restart before and after telling; two editors), chat's editor
  tests and be-server's. Known gaps (guides/type_modules.md, "Announcements"):
  a follower's message is told by the follower's replica once the owner accepts
  it, so a follower that closes or restarts before then tells nobody, as its
  unsent edit is lost too; and an owner whose append the server took but whose
  answer a disconnect lost, after which another device took the session over,
  never tells it.
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
- **Authorship.** The editor signs each message and edit through the host's
  `sign` before sending it, and marks a message whose signatures `verify`
  does not answer verified: "not verified", "key changed" or "forged".
  Deletions and reactions are not signed yet (plans/authorship.md).
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
   - **1d.** Segments, closing and rewriting them. Done: a chat of thousands of
     messages opens reading only the header; scrolling back reads segments by
     range; an offline reaction to a message moved into a segment survives the
     merge; a forced hash mismatch reads the part instead.
2. **Notifications** for chat (landed): sending a message announces its text
   and the accounts it mentions (`LiveEdit::announcements`), which the host
   sends once the server holds the message. Done: a member who is not connected
   gets a notification, one who muted the chat none, one who wants only
   mentions one for a reply to them, exactly once whatever editors are open and
   across a restart (be-host-stack's `a_chat_message_*`); the device showing it
   names the chat and author once plans/notifications.md step 1 has its
   Chromium test.

## Not planned here

- Direct messages: a chat is made and shared like any block.
- Search: plans/search.md.
- Signing messages: plans/authorship.md.
- Message keys for deleting and a "post" access level:
  plans/deferred/chat_moderation.md.
- Freezing chat's format: once it has users.
