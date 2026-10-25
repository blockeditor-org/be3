# Frozen formats

Some of what be3 saves is frozen: every later build must read what any earlier
build saved, forever, without asking anyone to delete their data. Everything
else may still change freely.

## What is frozen

- **The server's data directory**: `metadata.sqlite` (its tables and columns,
  and how each value is written: ids as text, hashes as hex, kinds, roles,
  access levels (`know_exists`, `view`, `edit`, `manage`), parent kinds, `wrapped_by` as a key id or `account:{uuid}`,
  password and token hashes) and `objects/`, each object in `objects/ab/cdef…`
  named by the SHA-256 of its bytes. That covers organizations (company or
  personal) and memberships, workspace and other blocks with their sealed
  metadata and current data key, commits (with the time the server received
  each, `block_commits.received`, 0 before it was recorded), grants, invitations, contact links,
  contacts and key changes, the `keys` and `wrapped_keys` rows, pinned escrow
  keys, pending rotations and agents, and the server's own account
  (`server_account`: its account id and the secret of the key that blocks
  shared with the server are sealed to).
- **The server's backups**: `databases/*.sealed`, sealed with the backup key,
  and `objects/` laid out as on the server.
- **What the objects and rows hold**, all sealed so only clients read them:
  - chunks, commits and manifests (CBOR from `Vault::put_value`) and
    `BlockMetadata` (in a commit, and on the block's row), each sealed with
    its block's data key in the tagged form (`ContentKey::seal`: `be3K`, the
    key id, a nonce derived from the key and the plaintext, the ciphertext);
  - data keys wrapped under scope keys, and scope keys under their parent
    scope's keys, the same way; scope keys under the escrow key
    (`RecoveryPublic::seal_anonymous` to its public half), and the escrow
    chain itself (`EscrowKey`, each epoch signed by the one before over
    `EscrowKey::chained`);
  - keys sealed to an account by a sender (`RecoverySecret::seal_for`, the key
    id as context, opened with `open_from`), the identity key pair a recovery
    phrase's twelve words derive, and what an account seals for itself
    (`seal_own`: its contacts' `ContactCard`s, its links' codes, its pinned
    escrow keys, `seal_pin`);
  - what is sealed with an invite or contact link code (`InviteCode`: contact
    cards by role, a pending grant's key, an invitation's escrow key), whose
    lookup id names the row, and key changes (`seal_key_change`).
- **A block file** (`be_client::transfer`, version 1, what `be download` and
  the file tree's Export as write): plaintext ids, types, parents, encoded
  `BlockMetadata`, references and content bytes. It is how data moves from one
  server to another, so it is read forever, and it is what carried data
  across the clean break below.
- **The content of these blocks** (`Root::FROZEN`): `TextBlock`, `Folder`,
  `Workspace`, `Database`, `DatabaseSchema` and `DatabaseView`, and their
  content type ids.

The server's data, its backups and the sealed objects broke with the old ones
once, on purpose, when workspaces became organizations and keys moved to
blocks (a clean break agreed with the user: data moved across in block
files). What `crates/be-frozen`'s snapshots hold, from 2026-10-16 on, is read
forever.

Section 1 of `plans/settings.md` is a second, one-time break of the same kind:
its changes (metadata that points to a block, with `ARTIFACT` now pointing to
its source; the access model; settings blocks, their `SETTINGS` metadata and
the `ANYONE_INSIDE` grant on a kind) land together, and then the one
server with data is wiped and its content re-imported once from a block file.
Until then they may change other saved data incompatibly, but not what
`crates/be-frozen`'s snapshots and fixture hold (none of it carries
`ARTIFACT` metadata, which only the app writes), so those stay read forever
throughout; afterwards the rules below hold again for everything.

Not frozen yet: every other block type, the network
protocol (server and clients are upgraded together and refuse other builds),
the local plugin API, live-session state (operations,
`LiveEdit::session_state`, relayed messages, presence), and the app's own
files on a device (`app.sqlite3`, the refs database, caches).

Remote human session credentials from before the account portal are intentionally invalidated
when session metadata is introduced; users sign in again. Account data, password hashes,
encryption keys, local profiles and agent credentials remain readable.

## Changing a frozen format

Only add:

- **SQLite.** Add tables, indexes and columns with a default;
  `sqlite_schema::ensure` adds them to an existing database on open. Never
  drop, rename or retype a column, or change how a value in one is written.
- **CBOR values** (commits, manifests, metadata, registers). Add fields with
  `#[serde(default)]` and add enum variants; never rename or remove one.
- **Documents.** The stack guide's "The stored form": a renamed field keeps its
  stored name, and a reshape bumps the root's `format` with a `migrate`.
- **Keys and sealing.** Never change how an existing key is derived, a nonce is
  chosen or bytes are sealed. A new scheme needs a way to tell its data from
  the old, and the old stays readable.

## The tests

- The `tests/frozen_*.rs` of the crate that holds a frozen block type (`crates/editors/database/types`
  for the database types, `crates/editors/text-block/types` for text,
  `crates/editors/workspace-ui/types` for the folder and the workspace) hold its registry and a document it saved, checked
  by `be_model::schema::check_frozen` (the stack guide's "Freezing a
  version"), and a test there checks its registers read without any one field.
  Moving a type into its plugin moves these with it.
- `crates/be-frozen` holds `src/fixtures/tracker.block`, a block file the
  `block-file` example wrote with the code of 2026-10-16: a folder holding a
  text block and a tracker database (view, database, schema, two rows linked to
  each other, a row's page). Its test uploads it into a new server the way `be
  upload` does and reads the folder, the text and the rows back.
- `crates/be-frozen` is also where snapshots of a whole server go: a data
  directory (`data/`) and a backup of it (`backup/`, whose `objects/` are the
  data directory's), zipped in `src/snapshots/NAME.zip`, with the tokens,
  recovery phrases, link and invite codes and keys to open them in
  `src/snapshots/sNAME.rs`. The `freeze` example makes one by running a real
  server and writing `scene.rs`'s accounts and blocks through `be_client` and
  `be`'s tracker: a company organization (My files with notes, a Team
  workspace holding the tracker database shared with an agent, an
  administrator demoted so the escrow key rotated, a grant revoked so Team's
  scope key rotated, a pending invitation, and from 2026_10_25 the
  organization's settings shared with the server, holding a policy it
  enforces) and the personal organization (a
  folder shared with a contact made by link, a pending link carrying a grant,
  and that contact's change of recovery phrase). The test serves each
  snapshot and reads it the way the app and `be` do: signing in by password,
  token and phrase, opening the keys, names, content and history, writing on
  an old commit, the contacts, the escrow chain, accepting the link and the
  invitation, and restoring the backup.

Never edit or regenerate a snapshot, a fixture, a `frozen_*.rs` module or a
value in `scene.rs`: a failure means the change broke old data, so fix the
change. Only the test's code may follow an API that is not frozen, such as a
protocol message.

When a frozen format gains something worth guarding (a column, a field, a
newly frozen block type), write it in the `freeze` example and `scene.rs`,
take a new snapshot and list it in `SNAPSHOTS` (`src/snapshots.rs`, with the
test that serves them):

```
./scripts/buck run //crates/be-frozen:example-freeze -- $PWD/crates/be-frozen/src/snapshots 2026_10_16
```

with the day's date as the name. The test then checks the new part only in
the snapshots that hold it. Freezing a block type also means `Root::FROZEN`, a
`frozen_*.rs` module and a `registers_accept_missing_fields` check of its kind
in its crate's tests, and a line in this guide.
