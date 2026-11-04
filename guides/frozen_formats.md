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
  keys, pending rotations and agents, which edit kinds were ever opened to
  anyone inside (`opened_kinds`), who uploaded each object in each
  organization (`object_uploads`, with its size, first upload time, the account
  it is charged to and whether a block holds it yet), storage accounting
  (`object_sizes`, `block_sizes` with each block's own and subtree size and who
  pays for it, `account_objects`, `account_usage`, `organization_usage`, and
  `server_backfills`, naming the one-time walks a server has done on open, such
  as the one that counts sizes for data from before accounting), invite codes
  (`invite_codes`, kept as hashes with their limits and uses, and the hash a
  pending registration in `account_mail` was made with, with the address that
  asked), and the server's own account
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
- **A block file** (`be_block_client::transfer`, version 1, what `be download` and
  the file tree's Export as write): plaintext ids, types, parents, encoded
  `BlockMetadata`, references and content bytes. It is how data moves from one
  server to another, so it is read forever.
- **The content of these blocks** (`Root::FROZEN`): `TextBlock`, `Folder`,
  `Workspace`, `Database`, `DatabaseSchema` and `DatabaseView`, and their
  content type ids.

What is read forever starts at the data `crates/be-block-frozen`'s snapshots
hold, from 2026-10-16 on; data saved before then is not read.

Section 1 of `plans/settings.md` is a one-time break: its changes (metadata
that points to a block, with `ARTIFACT` now pointing to its source; the access
model; settings blocks, their `SETTINGS` metadata and the `ANYONE_INSIDE` grant
on a kind) land together, and then the one server with data is wiped and its
content re-imported once from a block file. Until then they may change other
saved data incompatibly, but not what `crates/be-block-frozen`'s snapshots and
fixture hold (none of it carries `ARTIFACT` metadata, which only the host
writes), so those stay read forever throughout; afterwards the rules below hold
again for everything.

Not frozen yet: every other block type, the network protocol (server and
clients are upgraded together and refuse other builds), the local app API,
live-session state (operations, `LiveEdit::session_state`, relayed messages,
presence), remote human session credentials (a change may sign everyone out),
and the host's own files on a device (`host.sqlite3`, the refs database,
caches).

## Changing a frozen format

A frozen format changes only by adding to it, so that every earlier build's
data still reads:

- **SQLite.** Add tables, indexes and columns with a default;
  `be_block_sqlite_schema::ensure` adds them to an existing database on open. Never
  drop, rename or retype a column, or change how a value in one is written.
- **CBOR values** (commits, manifests, metadata, registers). Add fields with
  `#[serde(default)]` and add enum variants; never rename or remove one.
- **Documents.** The stack guide's "The stored form": a renamed field keeps its
  stored name, and a reshape bumps the root's `format` with a `migrate`.
- **Keys and sealing.** Never change how an existing key is derived, a nonce is
  chosen or bytes are sealed. A new scheme needs a way to tell its data from
  the old, and the old stays readable.

## The tests

- The `tests/frozen_*.rs` of the crate holding a frozen block type (`crates/be-type-database`,
  `crates/be-type-text`, `crates/be-type-workspace` for the folder and workspace) hold its
  registry and a document it saved, checked by `be_block_model::schema::check_frozen` (the stack
  guide's "Freezing a version"), and a test that its registers read without any one field.
  Moving a type into its app moves these with it.
- `crates/be-block-frozen/src/fixtures/tracker.block` is a block file written with the code of
  2026-10-16 (a folder with a text block and a linked tracker database); its test uploads it into
  a new server as `be upload` does and reads everything back.
- `crates/be-block-frozen/src/snapshots/` holds whole servers: a data directory and a backup of
  it, zipped, made by the `freeze` example from `scene.rs` (whose doc comment lists what is in
  it). The test serves each and reads it as the host and `be` do: signing in by password, token
  and phrase, keys, names, content and history, contacts, the escrow chain, links, invitations,
  storage (block and subtree sizes, each account's usage, an upload no block holds, the quota
  policy, an agent charged to its creator), invite codes, and restoring the backup. It serves
  with an upload grace longer than any snapshot's age, so a snapshot's pending upload is never
  collected. Snapshots from before storage accounting are counted by the server's backfill when
  they open, and the test checks those sizes add up too.

A snapshot, a fixture, a `frozen_*.rs` module and the values in `scene.rs` are what old data
looks like, so editing or regenerating one stops its test guarding that data. A failure means
the change broke old data: fix the change. Only the tests' code follows APIs that are not
frozen, such as a protocol message.

When a frozen format gains something worth guarding (a column, a field, a newly frozen block
type), add it to the `freeze` example and `scene.rs`, take a new snapshot named by the day's date
and list it in `SNAPSHOTS` (src/snapshots.rs):

```
./scripts/buck run //crates/be-block-frozen:example-freeze -- $PWD/crates/be-block-frozen/src/snapshots 2026_10_16
```

Freezing a block type also means adding it to `Root::FROZEN`, a `frozen_*.rs` module and a
`registers_accept_missing_fields` check in its crate's tests, and a line in this guide.
