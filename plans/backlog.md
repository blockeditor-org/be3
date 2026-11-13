# Backlog

Depends on: none.

The running to-do list, grouped. Every item says what "done" looks like.
Fixed items are removed as they land.

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

## 6. Handing the keyboard to a child block

The held-keys mechanism in be-host (an editor giving the keyboard to a child
block, such as a deck's notes; guides/apps/children.md, Focus) works but is
spread over three thread-local maps, two runtime lists and a `seen` flag.
Replace them with one gift record per editor instance in be-host-runtime: what
it is given to (a block, a child or a view), the keys held and whose they are,
moving pending → given → ready → done, with one function that ends it. Done
when:
- a child that is placed but never takes the keyboard ends the gift instead of
  holding the editor's keys until a press or a focus move;
- an entry left when the focus moved away before the child's view saw it can
  no longer replay its keys when that view is focused later;
- a block gift's deadline counts the asking instance's layout commits, not the
  whole app's, and a gift ends even if the editor commits no layout;
- held keys are released by the child's input-method request only if a frame
  was answered after the focus, so a stale `ime` can't release them early;
- a block gift ends when no child shows the block (a block gift already
  turns into a child gift at a layout commit, `settle_block_focus`, with unit
  tests in be-host-runtime's `instances/tests`).
