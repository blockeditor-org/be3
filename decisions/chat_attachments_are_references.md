# Chat attachments: references to what exists, children for what is new

A chat message's `attachments` name blocks. A block that already exists,
picked to attach, is a plain reference: it stays where it is, with its own
access, and is neither moved nor copied. Only content made for the message (a
block made in the picker, an image pasted into the composer) is created as a
child of the chat.

## Why

- Attaching should not move a block out of its folder, nor change who can
  read it: moving it under the chat would take it from its folder and revoke
  what its old folder's members could reach through it.
- A copy duplicates the block, so two versions drift apart, and an attachment
  of a large block doubles what is stored.
- What is new has no other home, so it lives with the chat and is copied and
  shared with it.

## Consequences

- A member who cannot read a referenced block sees a lock and "No access" in
  its place, not a link.
- A copy of the chat copies its children, not the blocks it references.
- A block made for a draft is a visible child of the chat before it is sent:
  the chat lists it among its drafts at once, so the server keeps it and every
  member can see and open it. The draft is kept with the chat, not the
  editor, so it survives a reload and closing the editor deletes nothing.
- A block made for a draft is deleted when it is removed from the draft while
  it is still the chat's child, and when a Not-sent message that alone carries
  it is discarded; a referenced block is left alone.

## Rejected

- Moving the picked block into the chat, and copying it into the chat.
