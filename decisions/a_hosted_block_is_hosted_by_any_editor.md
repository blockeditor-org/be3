# A hosted block is hosted by any editor

A block whose readers must not see all of it (a game world with secret state)
is run by a host: whichever member with Edit holds its live session. Others
reach it through a Join level that grants no keys, over a private channel to
the host (plans/hosted_blocks.md). Edit means "may change the stored block and
so may host it"; what a member may do inside the block is the app's own role,
kept in the content and checked by the host.

## Why

- It stays generic: share a world with one editor and two players, and either
  editor hosts while the players join, with no account or machine of its own.
- "Anyone with the link" (plans/share_links.md) can grant Join like any other
  level, to people nobody prepared anything for.
- A bound agent on a server machine is just an editor that stays online.

## Rejected

- **A block per player** that the player views and the host edits, with input
  as peer messages there: it works with today's levels, but a newcomer by link
  has no block yet and no way to ask for one, and it costs a key scope and a
  session per player.
- **A fixed authority**, one agent named as the block's host: only its
  machine could host, and the link would have to carry its key.
- **Server-enforced app roles** (a capability set wider than the levels): the
  server cannot read content, so it cannot express game rules or keep part of
  a block secret.
- **Roles checked by every reader** alone: it hides nothing from readers. It
  remains the likely shape for chat moderation, where no host should be
  needed (plans/deferred/chat_moderation.md).
