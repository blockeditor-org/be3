# be sync

`be sync` keeps a folder or workspace and a directory on disk in step, both ways. It is how code
that lives in be-host reaches tools that need real files: buck2, editors, CI and cloud agents.
It runs natively only.

```text
be Code sync ~/code                 the first time: Code (a folder or workspace) and ~/code
be sync ~/code                      again, with the block ~/code was synced with
be sync ~/code --watch [SECONDS]    keep going: on every change on disk, and every SECONDS (10)
```

The code is `crates/be-block-sync`, called by `be` (`crates/be-tool-cli`).

## What becomes what

be-block-sync knows no block format. Every block's file form comes from its type module
(guides/type_modules.md) through the `be_block_type_host::Modules` its caller passes, with the
same requests be-host uses for files opened in programs:

- `TypeInfo::plain` says whether a type is a `Directory` (folder, workspace), a `File`, or has no
  file form. Blocks with none (a canvas, a database) and everything inside them stay only in
  be-host, listed in `.be/unexported.txt`.
- A file's bytes are `to-file` of its block. A new file becomes a block of the first type that
  `claims` it by name and first 64 KiB, with `file` (any bytes) as the fallback; a changed file
  keeps its block's type and is applied with `FromFile`, so a text block keeps its language.
- The executable bit is the `EXECUTABLE` metadata key.

Sync does not join a live session: every change is a commit published against the head it read.
A replica open in be-host merges that like any outside publish, so the person keeps typing
(guides/type_modules.md).

## A pass

`.be/` at the sync root holds the index (`index.rs`), a lock keeping two syncs of one directory
apart, and `tmp/` for files being written. A pass:

1. Lists the block tree, a level at a time with each level's requests sent together, and walks
   the directory.
2. Gives every path a block id (`Pass::assign`), so renames and moves on disk stay moves of the
   same block.
3. Merges three `be_block_vcs::Tree`s keyed by block id, base (the index), ours (the disk) and
   theirs (the server), with `be_block_vcs::merge`, and merges files both sides changed with
   their type's `Merge::merge3`, or by parts for a type that merges by parts
   (guides/type_modules.md, "Merging by parts").
4. Applies the result to the server (`Peer::import_blocks`, `Peer::write`), then to the disk,
   then saves the index.

Nothing on disk is overwritten or removed unless it is still what the walk saw; a block whose
write was overtaken is merged again on the next pass.

## Names and ignores

A block is written only at a name valid on Linux, macOS and Windows and unique among siblings
ignoring case (`disk::name_problem`). Others are left off, never renamed, and listed with how to
fix them in `BE_SYNC_ERRORS.txt` at the root.

`.git/`, `target/` and `buck-out/` are always ignored, and `.gitignore` and `.beignore` apply per
directory as git reads them, `.beignore` first, so its `!pattern` brings back what `.gitignore`
leaves out. Both files sync like any other. A file that becomes ignored is left alone on both
sides. Symbolic links are left alone and reported.

## Watching

`--watch` runs a pass, then waits for a change on disk (`notify`, quiet for 200 ms) or the
interval. `be` speaks HTTP posts, which carry no server pushes, so changes made in be-host are
seen only by listing again each interval.

## Scale

`./scripts/buck run -m release //crates/be-block-sync:example-scale -- COPY WORK` starts a local
server and times syncing COPY. On this repository (about 6,500 blocks) on a 4-core cloud VM, a
first upload takes 10-14 s, a no-op or one-file resync about 0.25 s, and a first download about
4 s; time grows linearly with file count.
