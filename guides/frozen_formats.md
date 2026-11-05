# Frozen formats

Nothing be3 saves is frozen yet. No real data is stored, so every format
changes freely: the server's data directory and backups, what its objects and
rows hold, block files, block content, the network protocol, the local app API
and the host's own files. Don't keep backward compatibility for any of it: no
migrations, no renamed fields that keep reading their old names, no fallbacks
for data an earlier build wrote. A user or developer whose data an earlier
build wrote deletes it.

A block saved under another schema is still offered for reset
(guides/blocks/host.md, "Blocks saved by another build"), since every block
type is unfrozen.

The tools to freeze a format stay, with nothing using them, so freezing later
is a matter of adding data, not building machinery.

## Freezing a format later

A frozen format is one every later build must read, forever. It then changes
only by adding to it:

- **SQLite.** Add tables, indexes and columns with a default;
  `be_block_sqlite_schema::ensure` adds them to an existing database on open.
  Never drop, rename or retype a column, or change how a value in one is
  written.
- **CBOR values** (commits, manifests, metadata, registers). Add fields with
  `#[serde(default)]` and add enum variants; never rename or remove one.
- **Documents.** A renamed field keeps its stored name (`#[model(rename)]`,
  `#[model(alias)]`, `#[model(from_old(...))]`), and a reshape bumps the root's
  `#[model(format = N, migrate = path)]` (guides/blocks/content_types.md, "The
  stored form").
- **Keys and sealing.** Never change how an existing key is derived, a nonce
  is chosen or bytes are sealed. A new scheme needs a way to tell its data
  from the old, and the old stays readable.

## The tests

Each kind of test below exists with zero instances:

- **A block type.** Set `Root::FROZEN` (a frozen type is never offered for
  reset), save a document with `be_block_model::schema::freeze` into a
  `frozen_*.rs` module in the type's crate, check it with
  `be_block_model::schema::check_frozen`, and assert that
  `Kind::registers_accept_missing_fields` is empty.
- **Whole servers.** `crates/be-block-frozen/src/snapshots/` holds zipped
  servers (a data directory and a backup of it), made by the `freeze` example
  from `scene.rs` (whose doc comment lists what is in it) and listed in
  `SNAPSHOTS` (src/snapshots.rs), which is empty. Its test serves each and
  reads it as the host and `be` do: signing in by password, token and phrase,
  keys, names, content and history, contacts, the escrow chain, links,
  invitations, and restoring the backup. Take one with:

```
./scripts/buck run //crates/be-block-frozen:example-freeze -- $PWD/crates/be-block-frozen/src/snapshots NAME
```

- **Block files.** The `block-file` example of `crates/be-block-frozen` writes a
  `.block` fixture (a folder with a text block and a linked tracker database);
  a test that uploads it into a new server as `be upload` does and reads it
  back would guard it.

Once something is frozen, a snapshot, a fixture, a `frozen_*.rs` module and the
values in `scene.rs` are what old data looks like, so editing or regenerating
one stops its test guarding that data, and a failure means the change broke
old data. Freezing anything also means listing it in this guide and changing
the design principle in AGENTS.md.
