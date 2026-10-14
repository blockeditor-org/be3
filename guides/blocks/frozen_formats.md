# Frozen formats

What be3 saves that every later build must read forever, how to change it, and the tests that hold
it. Read it before touching the server's database or objects, backups, commits, metadata, key
derivation or sealing, or the content of a frozen block type. Everything not listed here may
change freely.

## Changing a frozen format

Only add. A test failure here means the change broke old data: fix the change, never the test's
data (see "The tests").

| Format | Allowed | Never |
|---|---|---|
| SQLite | Add tables, indexes, and columns with a default; `sqlite_schema::ensure` adds them to an existing database on open. | Drop, rename or retype a column, or change how a value in one is written. |
| CBOR values (commits, manifests, metadata, registers) | Add fields with `#[serde(default)]`; add enum variants. | Rename or remove a field or variant. |
| Documents | Keep a renamed field's stored name; bump the root's `format` with a `migrate` for a reshape (`guides/blocks/adding_a_block.md`, "Changing the stored form"). | Anything that stops an old document loading. |
| Keys and sealing | A new scheme, with a way to tell its data from the old; the old stays readable. | Change how an existing key is derived, a nonce is chosen or bytes are sealed. |

## What is frozen

- **The server's data directory.**
  - `metadata.sqlite`: its tables and columns, and how each value is written (ids as text, hashes
    as hex, roles, access levels, parent kinds, password and token hashes).
  - `objects/`: each object in `objects/ab/cdef...`, named by the SHA-256 of its bytes.
- **The server's backups.** `databases/*.sealed`, sealed with the backup key, and `objects/` laid
  out as on the server.
- **What the objects hold.**
  - Chunks sealed with the workspace key (`ContentKey::seal`).
  - Commits and manifests (CBOR from `Vault::put_value`).
  - `BlockMetadata`, in a commit and sealed on the block's row.
  - Workspace keys sealed to a recovery phrase (`be_keys`: the phrase's words and the secret
    derived from them).
- **The content of these block types** (`Root::FROZEN`), and their content type ids: `TextBlock`,
  `Folder`, `Database`, `DatabaseSchema`, `DatabaseView`.
- **Block URLs** stored in text (`guides/blocks/stack.md`, "Block URLs").

Not frozen yet:

- every other block type;
- the network protocol (server and clients are upgraded together and refuse other builds);
- the local plugin API;
- live-session state: operations, `LiveEdit::session_state`, relayed messages, presence;
- the app's own files on a device: `app.sqlite3`, caches.

## The tests

### Frozen block types

be-block's `tests/frozen_*.rs` hold each frozen type's registry and a document it saved, checked
by `be_model::schema::check_frozen` (`guides/blocks/adding_a_block.md`, "Freezing a version").

### Server snapshots

`crates/be-frozen` holds snapshots of a whole server:

- `src/snapshots/NAME.zip`: a data directory (`data/`) and a backup of it (`backup/`, whose
  `objects/` are the data directory's).
- `src/snapshots/sNAME.rs`: the tokens, recovery phrases and keys to open them.
- `src/snapshots.rs`: `SNAPSHOTS`, the list the test runs.
- `src/scene.rs`: the accounts and blocks written into each snapshot.

The `freeze` example made them by running a real server and writing `scene.rs`'s accounts and
blocks through `be_client` and `be`'s tracker. The test serves each snapshot and reads it the way
the app and `be` do, checking that:

- passwords, tokens and recovery phrases still sign in and open the workspace key;
- each block's name, content and history are as written;
- an agent reads the tracker;
- a new commit goes on top of an old one;
- the backup restores.

Never edit or regenerate a snapshot, a `frozen_*.rs` module or a value in `scene.rs`. Only the
test's code may follow an API that is not frozen, such as a protocol message.

## Guarding something new

When a frozen format gains something worth guarding (a column, a field, a newly frozen block
type):

1. Write it in the `freeze` example and `scene.rs`.
2. Take a new snapshot, named with the day's date:

   ```
   ./scripts/buck run //crates/be-frozen:example-freeze -- $PWD/crates/be-frozen/src/snapshots 2026_10_12
   ```

3. Add it to `SNAPSHOTS` in `src/snapshots.rs`.

The test then checks the new part only in the snapshots that hold it.

Freezing a block type also means:

1. `Root::FROZEN` on the type.
2. A `frozen_*.rs` module in be-block's tests.
3. Its kind in be-block's `the_frozen_blocks_read_every_register_without_any_one_field`.
4. A line in "What is frozen" above.
