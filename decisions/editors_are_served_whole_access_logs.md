# Editors are served whole access logs

A block's access log is a hash chain of signed records
(guides/blocks/keys.md, "Access logs"). Whoever holds Edit or above on a block,
or on a block below it, is served its whole log and the whole logs of the
blocks above it they may view; members with Edit on any block of a company
organization, and its administrators, are served the organization's whole log.
Everyone else with access (View, Join) is served a proof path: the entries
that prove their own right and that of whoever sealed them a key, the rest
reduced to what chains them.

## Why

Editors seal keys to others, and a sealer must see every entry up to the head:
an entry reduced to its hash could be the revocation of the person it is about
to seal to, or of whoever granted them, or the demotion of an administrator.
What a whole log shows an editor is mostly what access already shows them:
everyone granted above already reaches the block, and its managers list them.
Readers below Edit never seal, so a path, which hides other members' grants,
is enough for them to check their own key.

## Rejected

- **A commitment to the log's state** (a Merkle map of current levels beside
  the chain), so a sealer could check one member without seeing the rest, and
  a path would not reveal the log's length or where it lowered anyone. Simpler
  to keep one chain and serve it whole to those who seal.
- **Proof paths for sealers.** A path can hide a revocation or a demotion
  behind a reduced entry, which is exactly what a sealer must not miss.
- **Refusing a seal whose sender a reader's path does not prove.** The server
  picks whom a path proves, so a reader below Edit checks a sealer only as a
  verified contact, and a revoked editor, with the server, can hand it a key
  they made up (guides/blocks/keys.md, "Accepted limits"). Refusing instead
  would refuse every honest epoch made by an editor granted before any
  revocation; the reader seals and writes content under no such key.
