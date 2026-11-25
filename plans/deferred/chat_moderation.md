# Chat: final deletion and a post access level

Depends on: plans/signed_logs.md (built).

Set aside until the user picks them up.

## Final deletion

A redaction (guides/blocks/server.md, "Signed logs") drops an entry's payload
on the server and on every device that hears it, keeping its hash, and only
the entry's author or someone with Manage may redact. What it cannot reach is a
copy taken before, such as an export. A key per entry, wrapped under the block's data key and
deleted with the redaction, would make those copies unreadable too. Not
designed yet.

## A "post" access level

A level between View and Edit that may append to a block's log but not change
its content: one rule on `append` (plans/signed_logs.md, C4). Not designed
yet.
