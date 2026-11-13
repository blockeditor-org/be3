# Backlog

Depends on: none.

The running to-do list, grouped. Every item says what "done" looks like.
Fixed items are removed as they land.

## 2. Docking

- **Bar drops across layers.** A joined dock bar (guides/beui/docking.md,
  joined groups) dropped across layers only reorders within its own layer.
  Done: it can be dropped on any edge of any joined layer's pane, shown by a
  dock test.

## 4. The clipboard's last pieces

The typed clipboard is in (guides/beui/platforms.md, Clipboard and paste;
guides/apps/host_services.md, Copying and cutting blocks). Left of it:

- **Android.** be3 formats stay in memory there, matched by plain text. Done:
  they travel as `ClipData` MIME types, checked on an Android emulator
  (guides/running/android.md).
- **macOS.** The same. Done: macOS writes them as custom pasteboard types and
  knows another program copied by the pasteboard's change count. No Mac is
  available, so this is written blind: done when it compiles for macOS and
  shares its logic with tested code where it can.
- **Rich text marks in text fragments**, with `text/html` written from marks
  and read back into them. Marks travel as byte ranges within each chunk of
  `TextFragment` (`be-lib-ui-types/src/clipboard.rs`, a format change, which
  is fine). Marks whose keys belong to another schema are dropped on paste;
  embeds cross in the clip's `refs`, translated like any other. Done: a
  paste between two rich texts keeps bold, links and an embed, and pasting
  from a browser's `text/html` gives bold and links.

## 7. Sessions

- **Catch-ups from viewers.** `Catchup` is a `peer` relay, so any viewer can
  send one, and each makes the owner answer a `Snapshot` (with session state
  unless the asker already holds it) and may set it to reload. Done: the owner
  answers one sender at most once per interval, shown by a be-sdk-type-guest
  test where a viewer's repeated `Catchup`s get one `Snapshot`.

## 8. Input

- **Touches per window.** beui-core's `InputState::touches` is one list across
  the windows an input serves, so fingers on two touch displays at once read as
  one gesture, as a click in one window once counted toward a double-click in
  another (`ClickCounter`). Done: a touch in one window never joins a gesture in
  another, shown by a beui-core test with two windows' touches.

## 9. Version control's caches

- **Trees and commits read once.** A version command in the lane runs on a `Versions` of its
  own (`Served::version`), so it reads again every tree and commit the loop's statuses already
  hold, and what it writes never reaches theirs. Done: the loop's `Versions` and the lane's
  share one cache of trees and commits (behind an `Arc`), and a status after a command reads
  nothing the command already read.
- **A bounded walk to the merge base.** A version command's write of a block that moved
  (`version.rs`, `merge_over`) finds the two heads' common ancestor by walking all the history
  the device holds of each, up to four times a write. Done: the walk stops at a depth past
  which the command falls back to the head it read as the base.

## 10. Search

- **Compaction past the byte cap.** A writer's compaction makes no merge reading more than
  `MERGE_BYTES` (`SearchIndexContent::compact`), so a large index's biggest segments are never
  merged. An idle-time pass (the device idle, on power) could lift the cap. Done: a stack test
  where an index whose tier is over the cap is merged by an idle pass and answers the same.

## 10. Commit times

- **A refused far-future commit is stuck.** A device stamps commits by the
  server's clock it measured, kept in its refs across restarts
  (`Peer::server_now`), but one that has never connected stamps by its own
  clock, and the server refuses a time more than `MAX_COMMIT_AHEAD` ahead
  (guides/blocks/server.md, limits). Its outbox then holds a commit it cannot
  send until the server's clock catches up. Re-stamp it instead: when a publish
  is refused for its time, measure again and rewrite the unsent commit (and any
  unsent ones on it) with a new time, which changes their ids, so the journal,
  pending entries and local heads that name them move too. Done: a client test
  whose first commits are made offline with a clock two hours fast sends them
  once it connects.
