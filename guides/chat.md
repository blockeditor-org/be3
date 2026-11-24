# Chat

A chat block: a group conversation that is an ordinary block, made and shared
like any other (`crates/be-type-chat`, `crates/be-app-chat`). Its messages are
its signed log's (guides/type_modules.md, "Signed logs";
plans/signed_logs.md): each is an entry its author appended, which the server
ordered and stamped and the host signed as the chat app's, so a chat grows
without its content growing, and nobody but a message's author edits or
deletes it. What is not built yet is in plans/chat.md.

## The log

`ChatEntry` (postcard) is what an entry says; who wrote it, when the server
took it (its arrival) and who signed it are the log's (`LogItem`). A message
is named by its place in the log (its seq).

- `Message { time, body, reply_to, attachments }`: `time` is when its author
  says it was written; a message that arrived well after it (`OFFLINE_AFTER`,
  five minutes, in the editor) was written offline, and says when it arrived.
- `Edit { message, time, body }`: what the message says now; only its author's
  count, the latest by place.
- `React { message, reaction, kept, takes_back, time }`: each person's latest
  record of each reaction (a Lucide icon name) counts; taking one back appends
  a record that keeps nothing, names the one that kept it (`takes_back`) and
  redacts it.
- `Delete { message, time }`: only the message's author's counts; it redacts
  the message's entry, so its body is gone everywhere while the log still
  proves it existed and who wrote it.

`ChatLog::of` folds the entries an editor holds into what it shows (`Shown`):
each message with its author's latest edit, its kept reactions and its
author's deletion. A row's verdict is the least trusted of the entries it
shows (`least_trusted`); an entry signed as nobody's (`Signer::Bad`)
changes nothing, though a message so signed still shows, marked "forged". A
redacted entry its author took back (named by a later entry of theirs: a
deletion, a `takes_back`) shows only as the message the deletion names; one
redacted by anyone else (a manager, the server) shows in its place as
"Removed, not by its author" (`Shown::removed`). A deletion or take-back
naming another author's entry counts for nothing.

## The content

`ChatHeader` keeps what anyone with Edit may rewrite: the blocks the chat
lists as its children. `drafts` holds the blocks made for a message not sent
yet, with their author; `carried` the blocks sent messages carry, with the
message's entry id. Both are `LatestMap`s, so two devices' changes never
conflict, and neither undoes. A chat's children always come from its content,
never its log, so the chat's `LogFold` folds nothing (`FOLDS` false): its
replica neither reads nor keeps the log, and each editor reads what it shows.

## The editor

- It opens on the newest page of the log (`PAGE`, 100 entries): a read from
  past the log's end names its length (`ContentProjection::log_size`), then it
  reads the page before it. Scrolled to its first message, "Earlier messages"
  reads the page before what it holds. It keeps the latest
  `editor::LOG_HELD` entries, and all it paged back to.
- It sends, replies, edits and deletes its own messages and toggles reactions,
  each an `append` the host signs as the person's
  (`ContentProjection::append`). A draft stays in the editor until sent.
- **Sending.** A message the device has not sent shows as "Sending": the
  editor's own appends answered `kept`, and those a read of the log names
  (`LogPage::kept`), so it still shows after the chat or the host opens
  again. One the server refused shows "Not sent", with Send again (a new
  append) and Discard (`dismiss_refused`), which let the device's copy go
  too: the device keeps it until then, so one refused while the chat is
  closed shows when it opens again. A deletion, edit or reaction the server
  refused shows on its message's row instead ("Not deleted: why", "Edit not
  saved", "Reaction not saved"), all of a message's in one line under it,
  with Dismiss.
- **Who signed it.** Each row shows its verdict: "not verified" (a key no
  contact card vouches for), "key changed", "sent with an old key" (a key its
  author replaced before the server took it), "forged". A message its author
  deleted shows no row, unless the deletion's key is worth saying: "deleted
  with an old key".
- **Attachments** (decisions/chat_attachments_are_references.md). Attach opens
  the picker (`pick_block`, with templates, never the chat itself): an
  existing block it picks (`linked`) is a reference, left where it is with its
  own access; a block made in the picker is made in the chat, as is an image
  pasted into the composer (`TextInput`'s `on_image_paste`, `create_child`).
  A made block is listed at once in the content's `drafts` with its author
  (`make_draft`), so it is a live child before it is sent: every member sees
  it. The draft shows each with its type's icon and name, marks made ones as
  new, says so when what it carries is made here, and sends a made block only
  once it is one of the chat's children (`BlockQuery::Children`). Once the
  message's entry has an id (in the log, or sending), what it carries moves
  from `drafts` to `carried` (`ChatContent::send`), by whichever of this
  account's editors is open, so one that closed before its append was
  answered leaves it to the next; a message may be attachments alone, which
  has no text to edit. Each shows as a `BlockLink` under the message, typed
  from the chat's children and references; one this account cannot read shows
  a lock and "No access", not a link. The editor shows this account's
  `drafts_of` as chips when it opens, so they survive a reload, and drops a
  chip whose block left the drafts. Removing a made chip takes the block out
  of `drafts` (`drop_draft`, which deletes it) only while it is still the
  chat's child; the chat's `child_edit` deletes a draft and refuses a block a
  message carries. A message's deletion unlists what it carried
  once it is in the log (`ChatContent::delete`, by whichever of its author's
  editors is open), so one the server refused unlists nothing. A referenced block is never deleted.

## Notifications

A message sent tells the chat's members who are away: the editor appends it
with its announcement (`ChatEntry::announcement`, `Along::announcement`),
which waits in the device's outbox with the entry and goes once the server
takes it, mentioning the author of the message it replies to. Several taken in
one drain go as one ("{count} new messages"). Edits, reactions and deletions
tell nobody, nor does a message the server refused.

## Unread counts

Each device keeps where it read each chat up to, the place of the last entry
read, in the owner-only `ChatReads` (guides/settings.md), under its own key,
so devices moving on at once never meet and the furthest place counts. What
is unread is the messages others sent after that place and after the
account's own last message, since answering is reading (`ChatLog::unread`);
forged ones count for nothing.
The editor moves its device's place while the chat is shown at its newest
message with the keyboard. The host counts a chat it is not showing from the
log's latest entries (`be_block_content::UNREAD_ENTRIES`, guides/blocks/host.md,
"Unread counts") and counts again as the log grows.

## Imported history

An import keeps the exported chat's messages as history (`ChatLog::history`):
folded apart from the chat's own by their places in the chat they were
written in, so an imported edit, reaction, deletion or reply names only
imported messages, and none of the chat's own names one of them. The editor
shows it above the chat's own messages, read-only, under "Imported by
{importer} from an export", with no verdict on when a message arrived (the
file's word). An entry that may not be history here (`Bad`: not the log's
leading run at the place its importer signed, or not appended by the chat's
creator) is hidden. An export
carries only the chat's own messages, since a file's log holds what was
written in its block alone: exporting a chat with imported history leaves
that history out (plans/backlog.md, "Imported history in a re-export").

## Search

Each message is a document of the chat's log (`LogFold::search`): its own,
which its author's edit replaces and deletion drops, and any redaction drops
for good, so a message removed by someone else is not found either
(guides/blocks/host.md, "Search"). Reactions and forged messages are not
searched. An open chat hears a redaction as it happens (`Event::Redacted`),
so a removed message shows removed at once.
