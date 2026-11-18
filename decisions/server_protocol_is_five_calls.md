# The server protocol is five calls and one stream

`be3:server` is one interface: `read(organization, query, follow)`, `unfollow(organization,
topic)`, `write(organization, ops)`, `send(organization, target, payload)` and `events()`, beside
`ping` and the one-shot `device-login` (guides/blocks/server.md, "Protocol"). Every call names its
organization, none for what is the account's own; no organization is open on a connection. A
client signs in in the greeting, not by a call. What an account keeps for itself (contact
records, link codes, the escrow keys it pinned) lives in its own owner-only block in the personal
organization, not in server tables, and what the server or a contact leaves an account arrives as
a letter in its mailbox.

## Why

- **Atomic writes.** A write of several ops applies in one transaction, so a rule is checked once
  over all of it: a rotation is whole, a move into another scope leaves every moved key wrapped
  under the new scope's key, a removal carries every revocation it needs. That is the gain, not
  speed: calls on one connection are already answered in order and sent together.
- **Less server state.** What went is server state and code, not just operation names (76
  operations became about 8, which mostly renames): the lease and its timers, the hub's
  per-listing state and reference digests, the second revision counter, the open organization,
  the contact, link and notice tables and their four "read again" streams, and
  `collect-detached`.
- **Rules stay per case.** Who may make each query, op, target and topic is stated once each
  (`client::rule` in be-server), and a refused write names the op; only the dispatch shrank.

## Costs accepted

- **Re-reading trees.** A connection follows whole trees, not listings: one `tree` read per
  followed tree per burst of changes, whose pages carry changes in folders the device does not
  show, which it drops. Listings had sent only what a connection had listed.
- **A block known only through a reference** names no entry point, so it is read again only on
  `roots` or a reconnect (plans/backlog.md, "A block known only through a reference").
- **The server still learns each block's order of references** (`block_edges.position`), since
  a client diffs against it; a positionless reference change is the path if that should stop.

## Rejected

- One call per operation, with a stream per kind of change: more surface, and no transaction
  across calls.
- Signing in by the request's Authorization header alone: a browser cannot set a websocket's
  headers, so the token goes in the greeting, and the header only stands in when the greeting
  carries none (a proxy that injects it).
- A `Mailbox(account)` send target before any client writes a letter on its own: every letter so
  far is left by the server inside the write it tells, so none is lost between a write and a
  later send.
