# Chat segment rewrite: set aside

Depends on: none (chat segments are built, guides/chat.md).

Folding a closed segment's edits, deletions and reactions into a new part.
They stay in the header's maps, keyed by message id, which a rewrite would
shrink; a deleted message's body also leaves its closed segment then. A rewrite
is an operation that replaces an index entry (same first message, new length and
hash) and carries the new bytes, merged like a close: identical rewrites merge
into one, and differing ones stay side by side as overlapping closes do (guides/chat.md, "Merging").
