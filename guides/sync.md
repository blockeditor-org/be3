# be sync

`be sync` keeps a folder or workspace and a directory on disk in step, both
ways. It is how code that lives in block-app reaches the tools that need real
files: buck2, editors, CI runners and cloud agents. It runs natively only;
the web has no file system to sync.

```text
be Code sync ~/code                 the first time: Code (a folder or workspace) and ~/code
be sync ~/code                      again, with the block ~/code was synced with
be sync ~/code --watch [SECONDS]    keep going: on every change on disk, and every SECONDS (10)
```

The code is `crates/be-sync`, used by `be` (`crates/be-cli`). It knows no block
format: every block's file form comes from its type module
(guides/type_modules.md), through the `be_types_host::Modules` its caller hands
`sync` and `watch`. A folder export in the app (plans/daily_driver.md, phase 7)
should use the same requests rather than writing another mapping, as block-app's files opened in programs do (`to-file` to write a block out and `from-file` to take a program's save back, guides/desktop/linux.md).

## What becomes what

- A type's `TypeInfo::plain` says what it is on disk: `Directory` (a folder, and
  a workspace, which is only ever a root), `File`, or `None`. The sync root is
  the block named, and must be a directory.
- A file's bytes are `to-file` of its block's content (guides/type_modules.md): a text block's
  body, the data of an image, PDF, audio or file block (a file block holds any
  bytes, such as a crate's `snapshots/*.paint`).
- A new file on disk becomes a block of the first type that `claims`
  it by its name and first 64 KiB: an image, PDF or audio block by its
  extension (and a PDF's magic), a text block when it is UTF-8 without a NUL
  byte (its language from the extension), and a file block, the fallback that
  takes any file, otherwise. Its content is `apply` of `change.from-file` on no
  content, and its references and derived metadata are `describe`.
  A changed file keeps its block's type: `FromFile` is applied to the content
  it replaces, so a text block keeps its language and indentation. A new
  directory is a block of the type that claims directories (`Claims` with
  `directory`, a folder), its children added with `Change::Child`, and a
  directory's entries change the same way.
- `be sync` never joins a live session. Every change it makes is a commit
  published against the head it read (step 5 below). A session open in block-app
  takes that in as it does any publish from elsewhere (its replica's
  `Head { Moved }`, guides/type_modules.md): it merges the new head with what was
  typed but not yet sealed, line by line, so the person keeps typing. Lines both
  changed, or changed next to each other, come out between conflict markers
  (block-type-guest's `an_external_publish_merges_into_a_live_replica`).
- The executable bit is the `EXECUTABLE` metadata key, so `scripts/` stays
  runnable.
- Any other block (a canvas, a database) has no plain-file form. It and what
  is inside it stay only in block-app, listed in `.be/unexported.txt`.

## The index

`.be/index.json` at the sync root is what the last sync left, like git's index:
for each block its parent, name, type, executable bit, the head commit synced,
the SHA-256 of its file's bytes, and either where it is (a path, and a file's
size, modification time and inode) or why it is not on disk. `.be/lock` keeps two
syncs of one directory apart, and `.be/tmp` holds files being written, which are
renamed into place.

A pass:

1. Lists the root and every directory block below it, one level at a time, all
   of a level's requests sent together, and walks the directory.
2. Gives every path a block id: the id the index has for that path, or else a
   vanished path's id with the same inode, a server block not yet on disk at the
   same path, a vanished file with the same bytes, or the vanished directory most
   of its children came from. Anything else is a new block. This is what makes a
   rename or move on disk a move of the same block.
3. Builds three `be_vcs::Tree`s keyed by block id, base (the index), ours (the
   disk) and theirs (the server), with each file's content standing for the hash of
   its bytes, and merges them with `be_vcs::merge`. A file whose size, time and
   inode match the index is not read again, and a block whose head matches it is
   not fetched.
4. Merges a file both sides changed with its type's `Merge3`: the base is the
   content at the index's head commit, ours is the disk's bytes applied to it
   with `FromFile`, theirs the server's content, and the merge's `ToFile` is
   written to both sides. Text merges line by line, and lines both changed (or
   next to each other) come out between `<<<<<<< local` and `>>>>>>> remote`; a
   blob both changed keeps the disk's bytes, and the other is in the block's
   history. No merge loses either side.
5. Lays the merged tree out on disk (names, below), applies it to the server
   (new blocks through `Peer::import_blocks`, edits through `Peer::save_encoded`
   against the head it read, renames, moves, deletes, and each directory's
   entries through `Peer::publish_encoded`),
   then to the disk (moves through `.be/tmp`, removals, writes), and saves the
   index.

A block whose write fails or was overtaken (a rejected publish, a file changed
since it was read) keeps its old index entry, so the next pass merges it again;
`be sync` runs up to four passes. Nothing on disk is overwritten or removed
unless it is still what the walk saw.

## Names

Block names need not be unique, but files must be, so a block is only written
at a path it can hold on Linux, macOS and Windows: not empty, `.` or `..`, no
`/ \ < > : " | ? *` or control character, no trailing dot or space, not a
device name (CON, PRN, AUX, NUL, COM0-9, LPT0-9, with any extension), at most
255 bytes, not ignored, not already taken by something the sync does not keep,
and unique among its siblings ignoring letter case. A file already on disk at
its path keeps it; otherwise siblings that clash are all left off. Nothing is
renamed: each block left off is listed in `BE_SYNC_ERRORS.txt` at the root,
with why and how to fix it, and its index entry says it was left off, so its
absence on disk is never read as a deletion. A file made at its path becomes a
new block. The errors file, `.be` and the index are never synced.

## Ignored

`.git/`, `target/` and `buck-out/` anywhere, and whatever `.gitignore` and
`.beignore` files say, each for its own directory as git reads them; `.beignore`
is read first, so `!pattern` there brings back what `.gitignore` leaves out.
Both files are synced like any other. A block whose path is ignored is left off
like a bad name. A file that becomes ignored stops syncing but is left alone on
both sides. Symbolic links are left alone and reported.

## Watching

`--watch` takes the lock and runs a pass, then waits for a change under the
directory (inotify through the `notify` crate, ignoring `.be`, `.git`,
`target` and `buck-out`), quiet for 200 ms, or for the interval to pass. `be`
speaks HTTP posts, which carry no pushes from the server, so changes in block-app
are seen by listing again each interval.

## Scale

`./scripts/buck run -m release //crates/be-sync:example-scale -- COPY WORK`
starts a server on this machine and times a sync of COPY (which gets a `.be`).
On this repository (5,784 files, 32 MB, 6,552 blocks with the directories) on a
4-core cloud VM:

| | |
|---|---|
| first upload | 10-14 s, most of it the local server writing objects |
| listing every block, new connection | 64 ms |
| walking the directory | 26-35 ms |
| no-op resync | 0.23-0.28 s |
| one file changed | 0.24 s |
| first download to an empty directory and cache | 3.7-3.9 s |

Four copies (23,136 files) take four times as long at every step.
