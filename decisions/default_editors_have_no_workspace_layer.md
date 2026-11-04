# Default editors have no workspace layer; a block keeps its own preferred editor

Which editor opens a block is a user setting only. A block may keep its own
preferred editor (`PREFERRED_EDITOR` metadata, chosen from Opens in in a
block's menu in Files), which wins over the user's default.

## Why

Editors for one block type are views over the same data, so which one opens a
block is a viewer preference, and a workspace layer adds a second place to
look. Where an author's intent matters ("this database is a board"), it
belongs to that block, the way a canvas keeps its last profile on the
document.

## Rejected

- A workspace layer for default editors: a second place to look for a viewer
  preference.
