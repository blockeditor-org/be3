# Mutes are read by the server

How an account is notified about a block (`all`, `mentions`, `off`) is one
small mute the server reads: the `mutes` query, the `set-mute(block, mode)` op
and the `mutes` topic (guides/blocks/server.md, "Notifications"). The server's
rows are the only copy.

## Why

- The server leaves a muted account out of a block's pushes, so a muted block
  wakes none of that person's phones and the push service sees no traffic for
  it.
- One copy, kept where it is read, needs nothing to keep two in step.

## Rejected

- **Modes on the device alone**, the server pushing every viewer: a muted
  block would still wake a phone for every push, costing battery and showing
  the push service traffic for a block the person muted.
- **A copy in the account's own block** beside the server's rows: two copies
  to keep in step, and the server would still need its own to filter pushes.
