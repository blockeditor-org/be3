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
- **Arrival time.** The protocol half is built (guides/type_modules.md,
  "Arrival time"): the owner stamps a follower's operation through
  `LiveEdit::arrival`, and drops one that claims a stamp. Left for chat: keep the
  stamp in the message op (`arrival` for `ChatContent`); its inverse operations
  (undoing a delete, anything else derived from held content) must leave the
  stamp off, since undo goes through `edit`, which refuses a claimed arrival, so a
  restored message shows its arrival as unknown. Its `transform` must never copy
  another operation's arrival stamp into the one it rebases: the owner would take
  that rebased operation as claiming an arrival. A
  message shows its claimed time and, once stamped, if it arrived well after
  that, says it was sent offline and when it arrived. A message without a stamp
  shows no arrival, never its claimed time. A dishonest session owner could
  alter stored content, which plans/authorship.md accepts.
- **The owner's own messages unstamped.** The owner applies its own operation
  before the server has seen it, so it carries no stamp. Stamping it needs
  `append` to answer the server time it took the entries at, the same time it
  stamps their relays with, and a session message (`Stamped`, say) so every
  replica, and the snapshots and journal the owner hands on, hold the same
  stamp; otherwise replicas would disagree on content. Done: an owner's message
  shows when the server took it on every replica, checked by a guest test.
- **"Not sent" across a restart.** A refused commit's edit no longer shows
  "Not sent" after a host or module restart: `refused-commits` names commits,
  not edits. An `unsent_over` against the refused commit's parent would name
  them.
- **A host test of sending state.** `HostTest::offline` cuts the server, so a
  host test can cover it: send a message, wait for `synced`, go offline, send
  another and wait for "Sending" on it, then come back and see it go. Written
  that way it fails about one run in seven under `local-tools stress`, with
  nothing lost (a second device gets the message once the first is back): in
  the failing runs a drain publishes the offline message's kept commit with
  the peer attached, before the test brings the device back. Find whether the
  harness lets the device reach the server while offline (be-block-fault-relay,
  `drive offline`) or the device does, fix that, and land the test with a
  painting. Done: it passes 50 runs of 50 under stress.
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
