# One clipboard for every editor

Copy and paste, and drag and drop, carry text and PNGs only, so each editor
works around it. This plan gives the host one clipboard that carries typed
data: text fragments, canvas items, block references. Not implemented yet.

## What exists

- **beui.** `copy_text` and `copy_image` write; a node lists the `PasteKind`s
  it takes (`Text`, `Image`) and gets a `Pasted` (guides/beui/platforms.md,
  Clipboard and paste).
- **App protocol.** `ShellCall::CopyText` and `CopyImage`; pastes arrive as
  text in pieces (`paste_events` in `be-sdk-api`).
- **The canvas** copies its items as plain text, `be3-infinite-canvas:{json}`,
  with each layer a base64 PNG inside (`be-app-infinite-canvas/src/root/state.rs`).
  Other programs paste the JSON.
- **The text core** keeps a private `clipboard_cache` (`copy_utf8` and `paste`
  in `beui-text-editor-core/src/core.rs`): the raw bytes per cursor and whether
  whole lines were copied (an empty selection), used when the pasted text
  equals what it copied. So invalid UTF-8, one chunk per cursor and pasting a
  line on its own line survive a copy only within one core.
- **Dragging between editors** is the host's own typed path,
  `Dragged::{Block, Files}` (`be-sdk/src/host.rs`).
- **The compositor** reads the text of a program's selection as soon as it is
  set and hands it to be-host, so the text outlives the program; a selection
  of images only is not handed over (guides/desktop/wayland.md).

## The design

### One clip for paste and drag

A clip is a list of representations of the same content, richest first, each
a format id and its data: `be3/canvas-items`, a PNG of the same items, their
text. Drag and drop carries the same clip; `Block` and `Files` become formats.

### Formats

- be3 formats are Rust types with serde in one shared crate:
  `be3/text-fragment`, `be3/blocks`, `be3/canvas-items`. They are not frozen.
- Standard formats (`text/plain`, `text/html`, `image/png`, files) map to each
  platform's own.
- The receiver chooses: `pastes` lists format ids and the paste brings the
  first the clip holds. Ctrl+Shift+V pastes plain text.

### The host owns the clipboard

- Apps copy and paste through the host. The protocol carries
  `Copy { clip }` and pastes of the chosen representation in pieces, plain
  data, no framework types.
- The host drops its clip when another program copies: the compositor knows;
  Windows and macOS have a change counter; on the web the plain text is
  compared with what the host wrote.
- be3 formats go on the system clipboard where the platform does it well:
  any MIME type as compositor, registered formats on Windows, custom types on
  macOS, `ClipData` MIME types on Android, `web `-prefixed types on Chromium.
  On Firefox and Safari they stay in memory, matched by their plain text.

### What survives the app that copied

- What the app has at copy time is sent to the host then: the be3 format,
  text, images it already holds.
- Renders only the app can compute (canvas items as a PNG, markdown as HTML)
  stay lazy and are dropped when the app stops.
- Renders the host can make (a block's exported file through its type module)
  stay lazy for as long as the clip lives.
- The canvas format refers to its layers rather than embedding base64 PNGs, so
  it stays small.
- The compositor also takes images from a program's selection, so they outlive
  the program as text does.

### Text fragments

`be3/text-fragment` holds one chunk of raw bytes per cursor and a shape,
`Chars` or `Lines` (later perhaps `Block`, for column selections, and the
marks of rich text). Its plain text is the chunks joined with newlines,
decoded lossily. The receiver decides what a shape means: a text editor puts
`Lines` on its own line, the hex view inserts the bytes, a single-line input
keeps the first line. The core's copy returns a fragment and its paste takes
one; `clipboard_cache` goes.

### Blocks

- Copying a block copies a reference: `be3/blocks` holds block ids, their
  organization and type. Pasting adds a reference and leaves the parent alone.
  Plain text is a link and the title; a block with a file form offers its
  exported file.
- Cut is recorded by the host, not in the clip: the block, where it was cut
  from, and whether that place was its parent. Pasting moves the parent to the
  target only if the place it was cut from was its parent; cutting a mere
  reference moves nothing. The first paste uses up the record, so later pastes
  add references. A clip another program wrote can only ask for a reference,
  checked against access.
- A block cut and never pasted is deleted, as any block its parent no longer
  shows.

### Drag and drop

The action (move, copy, reference) is picked for each context, from what the
target accepts and the modifiers held. Follow the platform conventions, and
where conventions compete, pick the better one. Never ask with a menu.

## Work by layer

- beui: the clip type replacing `copy_text`, `copy_image`, `PasteKind` and
  `Pasted`; format ids in `pastes`; one clip type for `Draggable` and paste.
- Platforms: offer and read several formats; lazy representations; detecting
  another program's copy.
- App protocol and be-sdk: `Copy { clip }`, pastes and drags carrying a
  representation.
- be-host: the clip, cut records, resolving `be3/blocks` with access checks,
  type-module exports on request.
- be-host-wayland: take image selections too.
- The text core, canvas and hex view move onto their formats; the canvas
  prefix and the core's cache go.

## Decided

- A whole line (`Lines`) pastes at the start of the cursor's line, above it,
  and the cursor stays where it was: today's behaviour, kept.
- The drop action for each source and target is the implementer's to choose,
  by the rule in Drag and drop.

## Open

- Whether the first text fragment carries rich text's marks.
