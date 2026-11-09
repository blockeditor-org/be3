# Backlog

Depends on: none.

The running to-do list, grouped. Every item says what "done" looks like.
Fixed items are removed as they land.

## 1. Push, don't poll

Each piece of host state the editor mirrors already notifies its own
subscribers (`Mirror` in `be-sdk-beui/src/editor.rs`), but `Editor::begin_frame`
still compares the host's change counter (`host().changes()`) every frame
before syncing the mirror. Make it fully push-based, so a frame does no
checking at all. Done: a test with a host that counts calls shows a frame with
no pushes calls nothing on the host.

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
- **Paste in the files pane.** `be-app-file-tree` copies and cuts blocks
  (`root/menu.rs`) but takes no paste. Done: Paste on a folder places a
  `be3/blocks` clip there through `paste-blocks`.
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
  from a browser's `text/html` gives bold and links. Text editing on the
  shared sequence is plans/text.md; land this apart from its items.

## 5. Loading part of a document

Documents save as parts chosen by their structure but still load whole
(guides/blocks/content_types.md, "The stored form"). Let the model load part
of one: an attribute such as `#[model(lazy)]` on a `List` whose parts' objects
load when first read, through the `read-part` path a `Loads::FirstPart` type
already uses. Saving hands an unloaded part back as `Reused`, which keeps its
node's own references (`Part::Node { references }`,
`be-block-model/src/parts.rs`), so such a document drops only the unloaded
parts' references from `references_in`. Done: a document with a lazy list opens
without reading that list's parts, and saving it reuses them with their
references. Until this lands, documents merge as whole bytes (`merge3`), not by parts
(guides/type_modules.md, "Merging by parts").

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
- a unit test covers a block gift turning into a child gift at a layout
  commit, and ending when no child shows the block.
