# Chat attachments are copies

Attaching a block to a chat message copies it (`host().copy_blocks`), and the
copy moves into the chat as a child with the message that names it; the block
picked stays where it was. Until then the copy waits in Unfiled, since a child
the chat's content does not list counts as deleted.

## Why

- Attachments are child blocks of the chat, so the chat's members can open
  them and a copy of the chat carries them. Anything else needs the block to
  live under the chat.
- What is sent in a chat is a snapshot: later edits to the original should not
  change what was sent, as with a file attached to an email.
- A copy leaves the original's place and access alone.

## Rejected

- **Moving the picked block into the chat.** It takes a file out of its
  folder and silently revokes the access its old folder's members had through
  it, which nobody expects from "Attach".
- **Referencing the block where it is.** The chat's members may have no access
  to it, so the message would show a link most of them cannot open, and its
  content could change after it was sent.
