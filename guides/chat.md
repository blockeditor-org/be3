# Chat

A chat block: a group conversation that is an ordinary block, made and shared
like any other (`crates/be-type-chat`, `crates/be-app-chat`). Chat grows
forever, so it loads only what is on screen (a type can load only its first
part, guides/blocks/content_types.md "Parts"), and it is edited by many people
at once, so it saves less often than it is edited (the journal,
guides/blocks/client.md). What is not built yet is in plans/chat.md.

## The content

- **Written by hand**, as `Blob` is: `ChatContent` wraps the header
  `Document<ChatHeader>` (its first part, `Loads::FirstPart`) and the segment
  parts, and forwards `LiveEdit`, `Merge` and `Undo` to the header. It is not a
  derived `Document`, since a `Document` loads whole.
- **The header** holds the recent messages (a `merge_appends` list), `edits`,
  `deletions` and `reactions` (`LatestMap`s keyed by message id, on recent and
  older messages alike, kept apart from the messages so closing a segment never
  drops one), and the `segments` index of the closed segments.
- **A message** is an object of registers, not `Text`: author, claimed time,
  body, the message it replies to, attachments (child blocks) and its author's
  signature. Its id is its `ObjectId`. It stores the hash of the last message
  its author saw (`seen`); a message's hash is SHA-256 over its signed bytes.
  `Message::signed_bytes`, `MessageEdit::signed_bytes`,
  `Deletion::signed_bytes` and `ReactionKey::signed_bytes` (`src/signed.rs`)
  define what a signature covers.
- Sending and deleting are not undoable; an edit by anyone but the message's
  author is ignored. Deleting removes the message with its edit and reactions
  and records the `Deletion`; the message stays in version history.
- **Access.** Posting needs Edit, which also allows changing others' messages.
  A grant must name a contact, so a chat only ever reaches contacts.

## Segments

- **Segments** are the parts after the header: about 64 to 256 KiB of encoded
  messages each, one part per segment, never changed once closed
  (`src/segment.rs`). The stored form is the header, then the segments in the
  order of the index; each index entry (`Segment`) holds the segment's first and
  last message, their times, its count, its part's length and SHA-256, and a
  Bloom filter of its message ids (`may_hold`), so an editor finds a segment's
  byte range from the index alone (`ChatContent::segment_ranges`) and checks
  what it read against it (`read_segment`). A replica holds a segment as bytes
  while it has them unsaved, or as the part it was saved as (`from_stored`
  takes the refs of the header's siblings); an editor sent only the header
  holds neither.
- **Closing a segment** is an operation, `ChatOp::Close`: the header's change
  (the oldest recent messages leave `messages`, the index gains an entry) and
  the segment's bytes, which every replica that applies it keeps, so an owner
  that takes the session over saves the part like any other. The bytes ride in
  the operation (at most 256 KiB, or one larger message, which closes alone)
  rather than being put as a part named by hash: they are messages the session
  already relayed, so nothing is uploaded early and no hold is needed. Any
  attached editor proposes it (`ChatContent::closing`) once the recent messages
  pass 96 KiB, taking the oldest ones the server holds (not the sending ones),
  at least 64 KiB of them and leaving at least 32 KiB recent. A segment closes
  once, since the messages it names must be the oldest recent ones as written;
  a second proposal, or one over a message deleted meanwhile, is ignored by
  every replica alike. A chat nobody has open does not close segments.
- **Closing offline.** An editor proposes a `Close` only while every edit of
  its replica is confirmed (its lacking set is empty), so a device holds at most
  one unconfirmed close. A replica applies a `Close` only when its index entry
  is the one its bytes give (count, first and last message, length and hash)
  and its edit does no more than remove those messages and add the entry. The
  journal bounds how many closes a session carries (`MAX_JOURNAL_BYTES`, 4 MiB:
  about a dozen).
- **Limits.** A segment's Bloom filter can name a message it does not hold
  (unread counts may read one segment too many, never too few). A replica
  holds a closed segment's bytes in memory until it reloads
  (guides/type_modules.md, "Parts named by hash"; plans/deferred/chat.md). A deleted message's body stays
  in its closed segment (plans/deferred/chat_segment_rewrite.md). A chat
  rebuilt from the header's bytes (a view) saves the header alone, never a
  placeholder part. Duplicate, export, `be` transfer, version control's
  re-listing of a parent and revision preview or restore refuse a block of a
  first-part type that has parts after its first, until a copy of parts exists.

## Merging

- **Messages.** A `List` merge puts an insert after the sibling it was made
  after, except in a `merge_appends` list such as chat's messages, which keeps
  the published side as it is and puts what only the merging device added after
  it, in its order (guides/blocks/content_types.md). In a live session the owner
  sequences each send, an insert at the end, so a message lands after those
  sequenced before it, the same order. Edits, deletions and reactions are
  `LatestMap`s, so the later write of a key wins and none conflicts.
  Publishing is a compare-and-swap on the head: a device whose publish is
  rejected merges again on the new head, so a merge never has to give the same
  result from both sides.
- **By parts** (`MergeParts`): the header merges as a document and each segment
  of the merged index keeps the part of the side that has it, reused as saved,
  so a merge reads none. A segment closed on one side while the other reacted
  to, edited or deleted a message in it merges clean, since those are kept by
  message id.
- **Closes on both sides.** A segment both sides closed is one; when we closed
  segments they did not, ours stand and theirs that differ leave the index,
  their messages that are not in ours' segments (and not deleted) returning to
  the recent messages in front of them. That needs the parts of both, so it is
  `merge_parts`, which every merge of a chat takes (outside a session, `be
  sync`, and the replica's resume: guides/type_modules.md, "Merging by parts"),
  waiting while the parts are read; when a part cannot be read it is left undone
  and tried again on a timer, never resolving without it. `merge3`, which only a
  context with no parts runs, is the union: entries that differ stay, each with
  its own part, and can overlap, which loses nothing; the editor shows each
  message once, in its place (`ChatContent::weave`). Closing is not undone by a
  later merge. The index keeps entries sorted by where they end (then where
  they open), so the entry nearest the recent messages is read first and what
  shows stays contiguous while history is partly read.

## The editor

- It opens the header and shows the recent messages. Scrolled to its first
  message, or on "Earlier messages", it reads the newest segment it has not read
  (`FromEditor::Read` by the index's byte range; guides/blocks/apps.md,
  "Reading a range"), checks it against the index and shows its messages above
  the recent ones; what was read follows the edits, deletions and reactions
  made since.
- It sends, replies, edits and deletes its own messages and toggles reactions
  (Lucide icon names). A draft stays in the editor until sent.
- **Attachments.** Attach picks a block (`pick_block`) and moves it into the
  chat as a child (`set_parent`), so it is shared and copied with the chat;
  the message lists it in `attachments`, and a message may be attachments
  alone. Each shows as a `BlockLink` under the message, named and typed from
  the chat's children (`BlockQuery::Children`), opening the block on a press.
  Removing a picked block from the draft leaves it in the chat.
- **Sending state.** A message shows "Sending" while the server lacks the edit
  that sent it, and "Not sent", with Send again and Discard, once the server
  refused the commit holding it (`ContentProjection::lacking`,
  guides/blocks/apps.md "Sending"): the replica keeps those edits until the
  server holds them, across editors and replicas. For an owner alone that is
  every send for `APPEND_INTERVAL` (250 ms) and a round trip, even online. A
  host or type module restart keeps a commit still in the outbox as sending,
  but loses edits not yet in a commit (a journal entry the server lacks).
  be-host-stack tests and chat's editor tests cover it; `drive` cannot cut the
  server.
- **Authorship.** It signs each message and edit through the host's `sign`
  before sending it, and marks a message whose signatures `verify` does not
  answer verified: "not verified", "key changed" or "forged"
  (guides/blocks/keys.md, "Signing what a person writes").

## Notifications

Each message sent from this device tells the chat's other members once the
server holds it, exactly once, tied to the edit rather than to an editor or
replica (guides/type_modules.md, "Announcements"): `LiveEdit::announcements`
names, as the message is sent, its body and the author of the message it
replies to as a mention, keyed by the message. So it goes once however many
editors are open, or none (a chat closed while its outbox drains), after a
restart with the message unsent, and not again after a restart once told; a
message the server refused, or one discarded, tells nobody. Editing tells
nobody. A burst drained from the outbox is one notification, "N new messages".
The server tells only members with no connection open, none whose mode for the
chat is "off" (and for "mentions only", only mentions), and not the sender's
account (guides/blocks/server.md, "Notifications"). Tests: be-host-stack's
`a_chat_message_*`, chat's editor tests and be-server's.

## Unread counts

- Where the account read each chat up to is theirs, never in the chat, where it
  would be a read receipt: the owner-only settings kind `ChatReads`
  (guides/settings.md, Owner only), one place per device, so a device behind or
  offline never takes another's reading back. The furthest place counts, by
  message order: a deleted message's `Deletion` names the one before it. Each
  place is a key of its own (`chat/device/message`), and a device moving on
  drops only places it has seen that are not further than its new one.
- `ChatContent::unread` counts others' messages after it and after my own
  newest message (answering is reading), reading only their authors. The newest
  message and the counts come from the header, so a list of chats reads one
  small part per chat. A place that is not recent, not a deleted message's, but
  may be in a closed segment is before every recent message, so each recent
  message from others counts as unread; a replica does not read a segment to
  count the unread messages inside it. A place in none of them is one this
  replica has not received yet: all read.
- The editor reads the chat while it is shown, its view has the keyboard and
  it is scrolled to its newest message; otherwise it shows "N new messages"
  above the message box, which scrolls to the end.
