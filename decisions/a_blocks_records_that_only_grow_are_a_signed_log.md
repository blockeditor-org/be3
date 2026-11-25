# A block's records that only grow are a signed log, not content

A block may keep a signed log beside its content (plans/signed_logs.md, option A;
guides/type_modules.md, "Signed logs"). The two hold different kinds of data, and a type
puts each record in the one that matches it:

- **Content** is a whole state that anyone with Edit may rewrite: a commit is the new state,
  and the server checks only that it descends from the head.
- **A log** is what its authors add: entries the server numbers, stamps with when it took
  them and hashes into a Merkle tree, each signed by its author, that no commit changes. An
  entry can be followed by another (an edit, a reaction, a deletion) and redacted by its
  author or a manager, which drops its payload and keeps its hash, but never taken out.

Chat keeps its messages, edits, reactions and deletions in its log, and its drafts and the
blocks sent messages carry in its content.

## Why

- Content can say who wrote a record (a signature) but never that a record is missing: a
  removal is signed by nobody, and a commit of an earlier state is a valid descendant. A
  member with Edit could roll a chat back, take a reaction away unseen, write any arrival
  time or delete a message with an old key in silence. In a log none of that is a commit's to
  do, and a device that kept a head detects a log cut or forked under it.
- Only the server writes an arrival, so a key's replacement is judged against a time its
  author could not choose, for every entry.
- The server pages a log, so a record type no longer grows one state forever: chat's
  segments, closes merged by parts and the copies they refused went with it, and a page
  anywhere in the log costs that page plus about 20 hashes to verify.
- Comments on any block, the tracker's history and a deterministic game's moves are the same
  shape and use the same log.

## Rejected

- **Chat's own table beside its block** (option B): the same server pieces under chat's name.
  Search, export, duplicate, version control and unread counts would each special-case chat,
  and every later record type would build another table.
- **The server refusing commits that drop signed records** (C1): a commit would list the
  hashes of the records it holds. Readers must check content against the list, offline
  merges must union lists, and arrival is still content: a log bolted onto whole states.
- **Version control's history** (C2): commits are sealed by whoever holds the lease, not per
  author, retention thins them, and a check would walk history on every open.
- **Keeping the journal instead of trimming it** (C3): it is tied to a session's build,
  numbered by the device offline, and folded into the next whole state.

## Consequences

- One storage kind more in server, client, host and the type-module protocol, and a type
  says which of its records are which. Content edits announce nothing: a notification goes
  with a log entry (`Along::announcement`).
- The server learns per entry who appended, when and how large, as it already learns who
  submits to a session and when.
- An export carries the entries with their signatures, verifiable against the original block;
  a duplicate starts an empty log.
