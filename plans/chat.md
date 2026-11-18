# Chat: what is left

Depends on: none.

The chat block, its segments, merging, sending state, notifications, unread
counts (in the editor and on block rows) and member names are built
(guides/chat.md).
What is left waits for chat's messages to move to a server-ordered, per-author-signed
log (plans/signed_logs.md, option A, step 5), which changes how each is done:

- **"Not sent" across a restart.** A refused commit's edit no longer shows
  "Not sent" after a host or module restart: `refused-commits` names commits,
  not edits. An `unsent_over` against the refused commit's parent would name
  them.
- **Notifications that never go** (guides/type_modules.md, "Announcements"): a
  message's replica hands its announcements to the host with the submission,
  so a replica that closes or restarts before it submits tells nobody, as its
  unsent edit is lost too. Read again: the host keeps a closed replica's
  unsubmitted edits (`keep_open_edits`) and the next replica announces them
  again; what is lost is a submission the server took whose answer a crash
  lost (the replica then skips its own edit and nothing moves its row past
  `Submitted`), and kept edits of a chat never opened again.
- **Copying parts.** Duplicate, export, `be` transfer, version control's
  re-listing of a parent and revision preview or restore refuse a chat with
  closed segments until a copy of parts exists. Bytes carry only the header
  (`ChatContent::encode`), so every path that copies through bytes or the type
  module (`publish_bytes`, import, `Change::Replace`) drops the segments, and a
  copy into another block must seal the parts again under its key.

## Not planned here

- Direct messages: a chat is made and shared like any block.
- Search: guides/blocks/host.md, "Search".
- Signing deletions and reactions: plans/authorship.md.
- Rewriting a segment: plans/deferred/chat_segment_rewrite.md.
- Message keys for deleting and a "post" access level:
  plans/deferred/chat_moderation.md.
- Freezing chat's format: once it has users.
