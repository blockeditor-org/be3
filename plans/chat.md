# Chat

Depends on: none. The one step left needs plans/notifications.md step 1 (a
device that shows a push); the server's side and the `notify` host call it uses
have landed.

A chat block: a group conversation that is an ordinary block, made and shared
like any other. Chat grows forever, so it loads only what is on screen
(a type can load only its first part, guides/blocks/content_types.md "Parts"),
and it is edited by many people at once, so it saves less often than it is
edited (the journal, guides/blocks/client.md). Steps 1a to 1d have landed.

## What exists

- **The type and editor (1a).** `crates/be-type-chat` and `crates/be-app-chat`.
  `ChatContent` is written by hand, wrapping the header `Document<ChatHeader>`
  (its only part, `Loads::FirstPart`) and forwarding `LiveEdit`, `Merge` and
  `Undo` to it. The header holds the messages (a `merge_appends` list),
  `edits`, `deletions` and `reactions` (`LatestMap`s) keyed by message id, and
  the `segments` index of the closed segments. Sending and deleting are not undoable; an edit by
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
- **Segments (1d)** are the other parts: about 64 to 256 KiB of encoded messages
  each, one part per segment, never changed once closed (`src/segment.rs`). The
  stored form is the header, then the segments in the order of the index; each
  index entry (`Segment`) holds the segment's first and last message, their times,
  its count, its part's length and SHA-256, and a Bloom filter of its message ids
  (`may_hold`), so an editor finds a segment's byte range from the index alone
  (`ChatContent::segment_ranges`) and checks what it read against it
  (`read_segment`). A replica holds a segment as bytes while it has them unsaved,
  or as the part it was saved as (`from_stored` takes the refs of the header's
  siblings); an editor sent only the header holds neither.
- **Closing a segment** is an operation, `ChatOp::Close`: the header's change
  (the oldest recent messages leave `messages`, the index gains an entry) and the
  segment's bytes, which every replica that applies it keeps, so an owner that
  takes the session over saves the part like any other. The bytes ride in the
  operation (at most 256 KiB, or one message larger than that, which closes alone; under what a session relays) rather than being put
  as a part named by hash: they are the messages the session already relayed, so
  nothing is uploaded early and no hold is needed. Any attached editor proposes
  it (`ChatContent::closing`) once the recent messages pass 96 KiB, taking the
  oldest ones the server holds (not the sending ones), at least 64 KiB of them
  and leaving at least 32 KiB recent; the segment closes once, since the messages
  it names must be the oldest recent ones as written, and a second proposal, or one
  over a message deleted meanwhile, is ignored by every replica alike. A chat
  nobody has open does not close segments.
- **Merging** a chat is by parts (`MergeParts`): the header merges as a document
  and each segment of the merged index keeps the part of the side that has it,
  reused as saved, so a merge reads none. A segment closed on one side while the
  other reacted to, edited or deleted a message in it merges clean, since those
  are kept by message id.
- **Not built: rewriting a segment**: plans/deferred/chat_segment_rewrite.md.
  A deleted message's body stays in its closed segment until then. A chat
  rebuilt from the header's bytes (a view) saves the header alone, never a
  placeholder part. Duplicate, export, `be` transfer, version control's re-listing of a parent and revision preview or restore refuse a block of a
  first-part type that has parts after its first, until a copy of parts exists.
- **Limits.** A message larger than the largest segment closes alone. A
  segment's Bloom filter can name a message it does not hold (unread counts
  may read one segment too many, never too few). A replica holds a closed
  segment's bytes in memory until it reloads; a hook that turns them into the
  saved part is not built.
- **A message** is an object of registers, not `Text`: author, claimed time,
  body, the message it replies to, attachments and an empty signature. Its id
  is its `ObjectId`. 1a also defines a message's hash (over its signed bytes)
  and stores in each message the hash of the last message its author saw, and
  the bytes a signature will cover, for messages, edits, deletions and
  reactions alike, are defined from the start (plans/authorship.md), so
  signing later changes no format. A draft
  stays in the editor until sent.
- **The editor** opens the header and shows the recent messages. Scrolled to its
  first message, or on "Earlier messages", it reads the newest segment it has not
  read (`FromEditor::Read` by the index's byte range), checks it against the
  index and shows its messages above the recent ones; what was read follows the
  edits, deletions and reactions made since.
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
- Closing offline. An editor proposes a `Close` only while every edit of its
  replica is confirmed (its lacking set is empty), so a device holds at most one
  unconfirmed close; an offline device may still close, and so may another, over
  the same messages. A replica applies a `Close` only when its index entry is
  the one its bytes give (count, first and last message, length and hash) and
  its edit does no more than remove those messages and add the entry. A chat
  merge never conflicts and reads no part (a merge runs on one device and only
  its result is applied): a segment both sides closed is one; entries that differ
  stay, each with its own part (matched by the whole entry), and can overlap,
  which loses nothing and costs storage. The editor shows each message once, in its place
  (`ChatContent::weave`). Closing is not undone by a later merge, but once one
  side has added an entry nothing compacts the overlap until a rewrite exists. Edits, deletions and
  reactions are kept by message id whether the message is recent or not, so an
  offline edit to a message that moved into a segment merges clean and stays in
  the header's maps. The journal bounds how many closes a session carries
  (`MAX_JOURNAL_BYTES`, 4 MiB: about a dozen), and each close grows the index by
  one entry, which every replica holds.
- **Arrival time.** The server stamps each relayed operation with when it
  received it, in the relay envelope (commits already have
  `block_commits.received`), and replicas store that time in the message. A
  message shows its claimed time; once stamped, if it arrived well after
  that, it says it was sent offline and when it arrived. A dishonest session
  owner could alter stored content, which plans/authorship.md accepts.

### Unread counts

- The newest message and the counts come from the header, so a sidebar of
  chats (not built) reads one small part per chat.
- With segments, `messages()` holds only the recent ones. A place that is not
  recent, not a deleted message's, but may be in a closed segment (its Bloom
  filter) is before every recent message, so each recent message from others
  counts as unread; a replica does not read a segment to count the unread
  messages inside it, so those are not counted. A place in none of them is one
  this replica has not received yet: all read.

### Deleting

- Deleting removes a message from the current state, and it stays in version
  history. Keys per message that make a deletion final are deferred
  (plans/deferred/chat_moderation.md).

## Order of work

1. **Notifications** (step 2) for chat: sending a message calls `notify` with its text
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
