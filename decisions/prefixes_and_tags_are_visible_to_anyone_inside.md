# Each settings kind chooses who reads it; prefixes and tags default to anyone inside

A kind's visibility is "anyone who can view a block inside" or "members only",
chosen by the block's managers (`VISIBILITY`, `KindVisibility`). A kind open
inside carries a View grant to the pseudo-account `be_block_graph::ANYONE_INSIDE`.
The prefix registry and tag definitions default to anyone inside; every other
kind defaults to members only.

## Why

A block shared alone must still show its tags and resolve `BE-ABC`. Opening
them reveals tag names and prefixes to everyone a single block is shared with,
which is why it is a setting rather than fixed.
