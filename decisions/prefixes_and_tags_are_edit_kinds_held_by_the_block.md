# The prefix registry and tags are children of the owner block, changed with Edit

The prefix registry and tags (`be_block_graph::EDIT_KINDS`) need only Edit to
change; every other settings kind needs Manage. These two are children of the
owner block itself, held by a metadata key each (`be_block::edit_kind_key`),
not kinds of its settings block, so they inherit Edit like any child.

## Why

Creating a database updates the prefix registry and tags are managed by
editors, so these two need Edit. As ordinary children they inherit Edit like
any child, and the server needs no pointer it can't read.
