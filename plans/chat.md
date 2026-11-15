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
- **Unread counts outside the editor.** No sidebar or block row shows a chat's
  count yet.
- **"Not sent" across a restart.** A refused commit's edit no longer shows
  "Not sent" after a host or module restart: `refused-commits` names commits,
  not edits. An `unsent_over` against the refused commit's parent would name
  them.
- **Notifications that never go** (guides/type_modules.md, "Announcements"): a
  message's replica hands its announcements to the host with the submission,
  so a replica that closes or restarts before it submits tells nobody, as its
  unsent edit is lost too.
- **Copying parts.** Duplicate, export, `be` transfer, version control's
  re-listing of a parent and revision preview or restore refuse a chat with
  closed segments until a copy of parts exists.

## Not planned here

- Direct messages: a chat is made and shared like any block.
- Search: guides/blocks/host.md, "Search".
- Signing deletions and reactions: plans/authorship.md.
- Rewriting a segment: plans/deferred/chat_segment_rewrite.md.
- Message keys for deleting and a "post" access level:
  plans/deferred/chat_moderation.md.
- Freezing chat's format: once it has users.
