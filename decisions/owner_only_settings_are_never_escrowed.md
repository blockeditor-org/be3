# Owner-only settings are never escrowed

A settings kind may be readable by its owner alone (`KindVisibility::Owner`).
Its block is made `owner-only`, a flag the server stores on the block for its
whole life: the block is its own scope root, its key is sealed only to the
author's identity, never wrapped under the scope above and never sealed to the
organization's escrow key, and the server lets no one else open, move or share
it, administrators included: an administrator sees that the block exists
(`KnowExists`, since the account's `Settings` references it) but cannot open
it. An administrator who deletes the account's settings, or the account, has
the block collected with them. It is made only in its author's own `Settings`
or settings profile, so no one plants one where someone else pays for it.

The key is told apart by its id (a version-8 UUID, `owner_only_key`), which
every seal of the key binds, so a server cannot have a device escrow it by
claiming it is ordinary.

## Why

An app's key for an outside service, or where a person has read a chat up to,
is that person's and nobody else's, in a company organization too. Escrow
exists so administrators can recover what members made for the organization;
these are not that. The mark lives on the block, not only on the Rust type,
because the server computes escrow work itself and enforces who opens what.

The cost is that losing the recovery phrase loses these settings; the settings
page says so, and an app holding a key just asks to connect again.

## Rejected

- Keeping the mark only in the kind's type: the server could not tell the block
  apart, would list its key as unescrowed and let administrators open it.
- An organization policy that escrows owner-only settings anyway: not now; it
  would make the visibility mean nothing the owner can rely on.
