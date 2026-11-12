# Chat: final deletion and a post access level

Depends on: plans/authorship.md.

Set aside until the user picks them up.

## A key per message

- Each body (and its edits) is sealed under its own key. Message keys are rows
  on the server, keyed by block and message id and wrapped under the block's
  data key. The author's own connection creates the row, and the server
  records which account did. Only that account, or someone with Manage, may
  delete it; deleting it makes every copy in history unreadable without
  touching a commit. Someone else removing a message only changes the
  document, so history can bring it back.
- A segment's message keys come in one batched call beside its part. In a live
  session the sender sends the key with the message. Offline, the row waits in
  the outbox with the commit.
- Rewriting history instead would change every later commit's id, since commits
  name their parents inside their sealed objects.

## A "post" access level

Signatures detect forgery but don't stop someone with Edit rewriting or
removing others' messages. A level between View and Edit that may only add
messages would. Not designed yet.
