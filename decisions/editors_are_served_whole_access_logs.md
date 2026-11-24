# Editors are served whole access logs

A block's access log is a hash chain of signed records
(guides/blocks/keys.md, "Access logs"). Whoever holds Edit or above on a block,
or on a block below it, is served its whole log and the whole logs of the
blocks above it they may view. Only a company organization's administrators
are served the organization's whole log; every other member, an editor
included, is served a proof path of it.
Everyone else with access (View, Join) is served a proof path: the entries
that prove their own right and that of whoever sealed them a key, the rest
reduced to what chains them. A whole log costs an editor each entry once:
devices keep what they checked and read only what was appended
(guides/blocks/keys.md, "Access logs"); a path stays one step per entry
(decisions/proof_paths_stay_one_step_per_entry.md).

## Why

Editors seal keys to others, and a sealer must see every entry up to the head:
an entry reduced to its hash could be the revocation of the person it is about
to seal to, or of whoever granted them, or the demotion of an administrator.
What a whole log shows an editor is mostly what access already shows them:
everyone granted above already reaches the block, and its managers list them.
Readers below Edit never seal, so a path, which hides other members' grants,
is enough for them to check their own key.

The organization's log is the exception: it names every member, and only an
administrator may list them, yet a block's creator holds Manage on it, so
serving it to editors served it to nearly every member. An editor does not
need it whole: it seals to administrators through the escrow key, and an
honest organization path keeps every administrator record, and every record
about anyone who ever administered it, whole, so no demotion hides behind a
reduced entry; and it seals to a block's grantees by the keys the block's own
log names, which it is served whole. An editor's path of the organization's
log still shows its length: how many entries it holds, so roughly how many
invitations, members and changes there have been, though not whom they name.

## Rejected

- **The organization's whole log to every editor.** It handed the member list
  to every member who had made a block, which `list-members` keeps to
  administrators.

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
