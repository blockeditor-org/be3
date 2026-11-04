# A database prefix resolves only through the workspace's prefix registry

`be_block::PrefixRegistry` maps each database to its prefix, read as prefix to
a list of databases, since a prefix is a label and need not be unique. It is
the only way a prefix such as `BE-ABC` resolves in `be` and the host; there is
no fallback search, and an ambiguous prefix says so instead of picking one.
Creating, re-prefixing or moving a database updates the registries.

## Why

Since #603 no client lists every block, so nothing can enumerate every
database prefix; since #600 a prefix is only a label, so one prefix may name
several databases.

## Rejected

- A fallback search for the prefix: no client can list every block.
- Picking one database for an ambiguous prefix.
