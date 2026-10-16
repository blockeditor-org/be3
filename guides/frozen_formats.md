# Frozen formats

Some of what be3 saves is frozen: every later build must read what any earlier
build saved, forever, without asking anyone to delete their data. Everything
else may still change freely.

## What is frozen

- **The server's data directory**: `metadata.sqlite` (its tables and columns,
  and how each value is written: ids as text, hashes as hex, roles, access
  levels, parent kinds, password and token hashes) and `objects/`, each object
  in `objects/ab/cdef…` named by the SHA-256 of its bytes.
- **The server's backups**: `databases/*.sealed`, sealed with the backup key,
  and `objects/` laid out as on the server.
- **What the objects hold**: chunks, commits and manifests (CBOR from
  `Vault::put_value`) and `BlockMetadata` (in a commit, and on the block's
  row), each sealed with its block's data key in the tagged form
  (`ContentKey::seal`: `be3K`, the key id, a nonce derived from the key and the
  plaintext, the ciphertext); keys wrapped under other keys the same way, and
  scope keys sealed to an account's identity key on `wrapped_keys` rows
  (`be_keys::RecoveryPublic::seal`, opened by the secret a recovery phrase's
  words derive).
- **A block file** (`be_client::transfer`, version 1, what `be download` and
  the file tree's Export as write): plaintext ids, types, parents, encoded
  `BlockMetadata`, references and content bytes. It is how data moves from one
  server to another, so it is read forever even when a server's own data is not.
- **The content of these blocks** (`Root::FROZEN`): `TextBlock`, `Folder`,
  `Database`, `DatabaseSchema` and `DatabaseView`, and their content type ids.

The server's data, its backups and the sealed objects broke with the old ones
on purpose when workspaces became organizations and keys moved to blocks (a
clean break agreed with the user: data moves across in block files), and grew
again with contacts. Until `crates/be-frozen` holds a snapshot of the new
formats, nothing checks them, so take one as soon as they settle.

Not frozen yet: every other block type, the network protocol (server and
clients are upgraded together and refuse other builds), the local plugin API,
live-session state (operations, `LiveEdit::session_state`, relayed messages,
presence), and the app's own files on a device (`app.sqlite3`, caches).

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

- be-block's `tests/frozen_*.rs` hold each frozen block type's registry and a
  document it saved, checked by `be_model::schema::check_frozen` (the stack
  guide's "Freezing a version").
- `crates/be-frozen` holds `src/fixtures/tracker.block`, a block file the
  `block-file` example wrote with the code of 2026-10-16: a folder holding a
  text block and a tracker database (view, database, schema, two rows linked to
  each other, a row's page). Its test uploads it into a new server the way `be
  upload` does and reads the folder, the text and the rows back.
- `crates/be-frozen` is also where snapshots of a whole server go: a data
  directory (`data/`) and a backup of it (`backup/`, whose `objects/` are the
  data directory's), zipped in `src/snapshots/NAME.zip`, with the tokens,
  recovery phrases and keys to open them in `src/snapshots/sNAME.rs`. The
  `freeze` example makes one by running a real server and writing `scene.rs`'s
  accounts and blocks through `be_client` and `be`'s tracker, and the test
  serves each snapshot and reads it the way the app and `be` do.

Never edit or regenerate a snapshot, a fixture, a `frozen_*.rs` module or a
value in `scene.rs`: a failure means the change broke old data, so fix the
change. Only the test's code may follow an API that is not frozen, such as a
protocol message.

When a frozen format gains something worth guarding (a column, a field, a
newly frozen block type), write it in the `freeze` example and `scene.rs`,
take a new snapshot and list it in `SNAPSHOTS` (`src/snapshots.rs`, with the
test that serves them):

```
./scripts/buck run //crates/be-frozen:example-freeze -- $PWD/crates/be-frozen/src/snapshots 2026_10_12
```

with the day's date as the name. The test then checks the new part only in
the snapshots that hold it. Freezing a block type also means `Root::FROZEN`, a
`frozen_*.rs` module, its kind in be-block's
`the_frozen_blocks_read_every_register_without_any_one_field`, and a line in
this guide.
