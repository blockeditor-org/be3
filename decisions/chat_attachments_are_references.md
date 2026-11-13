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

- A member who cannot read a referenced block sees its link as "No access",
  greyed and not openable.
- A copy of the chat copies its children, not the blocks it references.
- A block made for a draft is deleted when it is removed from the draft, when
  the draft is left, and when a Not-sent message carrying it is discarded; a
  referenced block is left alone.

## Rejected

- Moving the picked block into the chat, and copying it into the chat.
