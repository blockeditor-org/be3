# Backlog

Depends on: none.

The running to-do list, grouped. Every item says what "done" looks like.
Fixed items are removed as they land.

## 2. Docking

- **Bar drops across layers.** A joined dock bar (guides/beui/docking.md,
  joined groups) dropped across layers only reorders within its own layer.
  Done: it can be dropped on any edge of any joined layer's pane, shown by a
  dock test.

## 3. Anchored views

- **Children over anchored views on the web.** A child placed in an anchored
  view with `ChildLayer::Above` shows under it on the web, since child screens
  are behind `#block` (guides/running/web.md). Done: it shows above it, as
  natively, checked by a host test on `:web` in Chromium; update
  guides/running/web.md.

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
- **Images beside text on desktops.** arboard (`beui-platform-winit`) sets
  either the plain text or a PNG, not both, so this needs per-platform code:
  X11 and Wayland selections offering both targets, and Windows clipboard
  formats. Done: on Linux, a test against a headless Wayland or X server
  (Xvfb, installed with apt on the test machine and started by the test,
  skipping with a message when absent) reads both targets from one clip;
  Windows compiles.
- **Block ids inside any clip, inside a checkout.** The host translates the
  ids of a clip crossing the app protocol for an instance in a checkout
  (guides/blocks/version_control.md, "Scopes and local ids"), but only for
  `be3/blocks`, which it decodes by format (`be-host-runtime/src/calls.rs`).
  Any other format holding block ids, such as `be3/canvas-items` or rich text
  with embeds, crosses untranslated. Make it general: every clip item carries
  the blocks it names as a list beside its bytes (`refs`), and its bytes
  index into that list, as view state does (`ViewState { bytes, refs }`). The
  host translates `refs` for every format and decodes none; `be3/blocks`
  moves its ids there too, and so does `CanvasItems.canvas`.
  - `refs` is a field of the WIT `clip-item` (shared with beui's
    `Clip`/`ClipEntry` in beui-types), not a side channel.
  - Every path that carries a clip translates it: `Copy` and `DragOver`
    (`calls.rs` visits them today), the answer to `RenderClip` for an item
    copied with `data: None`, and an incoming paste or drop.
  - Where be3 formats go onto the system clipboard (web custom types,
    Windows registered formats, macOS pasteboard types), `refs` are stored
    with the bytes, so a paste read back keeps them. Done: copying canvas entities that embed blocks,
  and blocks, from outside a checkout and pasting them into a canvas inside
  it resolves every block there, and the reverse.
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

## 8. Version control's caches

- **Trees and commits read once.** A version command in the lane runs on a `Versions` of its
  own (`Served::version`), so it reads again every tree and commit the loop's statuses already
  hold, and what it writes never reaches theirs. Done: the loop's `Versions` and the lane's
  share one cache of trees and commits (behind an `Arc`), and a status after a command reads
  nothing the command already read.
- **A bounded walk to the merge base.** A version command's write of a block that moved
  (`version.rs`, `merge_over`) finds the two heads' common ancestor by walking all the history
  the device holds of each, up to four times a write. Done: the walk stops at a depth past
  which the command falls back to the head it read as the base.
