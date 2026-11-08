# Backlog

Depends on: none.

The running to-do list, grouped. Every item says what "done" looks like.
Items marked **(ask)** need a decision from the user during the work. Fixed
items are removed as they land.

## 1. Push, don't poll

Each piece of host state the editor mirrors already notifies its own
subscribers (`Mirror` in `be-sdk-beui/src/editor.rs`), but `Editor::begin_frame`
still compares the host's change counter (`host().changes()`) every frame
before syncing the mirror. Make it fully push-based, so a frame does no
checking at all.

## 2. beui scrolling

- **First scroll is short on Linux.** The first wheel event scrolls less than
  every later one.

## 3. Editor icons in the manifest

A manifest's `icon` is a Lucide icon font codepoint (`""`), so an editor
can only use an icon from the font every beui context carries. Let an editor
declare any icon: the build rasterizes it at a few sizes into the manifest as
alpha masks, which the host draws tinted with the theme, picking the size
nearest the one it shows. An editor that uses a `beui::icons` icon names it in
its source, and the build rasterizes that glyph. Done: no manifest holds a
font codepoint.

## 4. Anchored views

- **Bar drops across layers.** A joined dock bar dropped across layers only
  reorders within its own layer. Done: it can be dropped on any edge of any
  joined layer's pane.
- **Children over anchored views on the web.** A child placed Above the pixels
  of an anchored view shows under them. Done: it shows above them, as natively.

## 5. The clipboard's last pieces

The typed clipboard is in (guides/beui/platforms.md, Clipboard and paste;
guides/apps/host_services.md, Copying and cutting blocks). Left of it:

- **macOS and Android.** be3 formats stay in memory there, matched by plain
  text. Done: macOS writes them as custom pasteboard types and knows another
  program copied by the pasteboard's change count; Android carries them as
  `ClipData` MIME types.
- **Images beside text on desktops.** arboard puts either the plain text or a
  PNG on the system clipboard, not both. Done: a clip with both offers both.
- **Paste in the files pane.** It copies and cuts blocks but takes no paste.
  Done: Paste on a folder places a `be3/blocks` clip there through
  `paste-blocks`.
- **Ids in pasted canvas items inside a checkout.** `be3/blocks` ids are
  translated across the protocol, but the block ids inside canvas entities are
  not. Done: a canvas in a checkout pastes entities whose blocks resolve there.
- **Rich text marks in text fragments**, with `text/html` written from marks
  and read back into them. Marks travel as byte ranges within each chunk; keys
  mean something only under the same schema; embeds bring block references.


## 6. Loading part of a document

Documents save as parts chosen by their structure but still load whole
(guides/blocks/content_types.md, "The stored form"). Let the model load part
of one: an attribute such as `#[model(lazy)]` on a `List` whose parts' objects
load when first read, through the `read-part` path a `Loads::FirstPart` type
already uses. Such a document drops the references of parts it didn't load
from `references_in`, and `describe` (the graph mirror) adds each part's own
(`Part::Node { references }`). Done: a document with a lazy list opens without
reading that list's parts, and saving it reuses them.
