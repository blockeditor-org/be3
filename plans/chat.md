# Chat: what is left

Depends on: none.

The chat block, its segments, merging, sending state, notifications and unread
counts are built (guides/chat.md). What is left:

- **Member names.** Apps have no way to learn a member's name: the `contacts`
  watch is the shell's alone, so members other than you show as "Member" and
  their account's first digits, and a notification's mentions are only the
  replied-to author. Add a host service that names the accounts in a block's
  members (contacts by their card, others by what the server says, marked
  unverified), with no framework types.
- **Attachments.** They are in the model (child blocks) but the editor cannot
  add or show them.
- **Unread counts outside the editor.** No sidebar or block row shows a chat's
  count yet.
- **Arrival time.** The server stamps each relay with when it took it
  (`relayed.received`), and the owner hands that to `LiveEdit::arrived` for a
  follower's operation; chat keeps it in the message (the owner's own messages
  and those that travel as a part get none; commits have
  `block_commits.received`). A message shows its claimed time; once stamped, if
  it arrived well after that, it says it was sent offline and when it arrived. A
  dishonest session owner could alter stored content, which plans/authorship.md
  accepts.
- **Edits lost on a restart.** A host or type module restart loses edits not
  yet in a commit (a journal entry the server lacks), so they no longer show as
  sending; a content-level answer (what the local head adds over the server's)
  would close that, which is the user's call. A `drive` command that cuts the
  server would let a host test cover sending state.
- **Notifications that never go** (guides/type_modules.md, "Announcements"): a
  follower's message is told by the follower's replica once the owner accepts
  it, so a follower that closes or restarts before then tells nobody, as its
  unsent edit is lost too; and an owner whose append the server took but whose
  answer a disconnect lost, after which another device took the session over,
  never tells it.
- **Copying parts.** Duplicate, export, `be` transfer, version control's
  re-listing of a parent and revision preview or restore refuse a chat with
  closed segments until a copy of parts exists.

## Not planned here

- Direct messages: a chat is made and shared like any block.
- Search: plans/search.md.
- Signing deletions and reactions: plans/authorship.md.
- Rewriting a segment: plans/deferred/chat_segment_rewrite.md.
- Message keys for deleting and a "post" access level:
  plans/deferred/chat_moderation.md.
- Freezing chat's format: once it has users.
