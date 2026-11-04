# The hex view on the text area

The text block's hex view (`crates/be-app-text/src/hex.rs`) is a hand-painted
view of its own. This plan rebuilds it from beui's text area. The point is to
make `beui-text-editor-core` keep bytes that are not valid UTF-8 intact, and to
let people inspect them; a codepoint view is expected later on the same pieces.
Not implemented yet. The clipboard parts are in plans/clipboard.md.

## What exists

- **The hex view.** `HexView` shares the block's `TextAreaState` with the text
  view, so the document, undo and sync are already one. It paints every row of
  the file into one `Drawing` whenever the content or cursors change, and has
  its own hit-testing (`HexGeometry`), keys, pending nibble
  (`hex_pending_nibble`), selection anchor and insert/overwrite switch
  (`hex_insert_mode`, faked as a one-byte selection). The only test is
  `switching_to_hex_view_shows_the_bytes`.
- **Rows.** The text area builds a row per source line (`line_starts`, which
  splits at `\n`, in `text_area/rows.rs`) in a `VirtualList`, caches built
  rows, and maps source to display ranges per row with `Segment`. Masking
  (`options.mask`) is the precedent for showing something other than the
  source: it pushes `•` with the source range of each character. The gutter
  shows line numbers.
- **Vertical movement.** `CursorHorizontalPositionMetric::Screen` carries a
  `VisualRows` (`beui-text-editor-core/src/core/rows.rs`): `caret_x(index)`
  and `step(index, direction, x)`, which answers the next row's bytes and the
  caret stop nearest `x`, `Edge`, or `Unknown`. The view decides what a row is;
  `wrapped_end` marks a row whose end is the next row's start (a soft wrap, a
  fixed-width chunk), so the caret settles a stop before it. The core keeps the
  goal x in `VerticalMoveStart`. The text area answers from its laid-out rows
  (`Surface::row_step`, published through `TextAreaLayout`) for Up/Down and for
  dragging past the viewport's edge; a row not built answers `Unknown` and the
  caret moves by source line. Home and End still go by source line.
- **Bytes.** A text block's body is a `Sequence<u8>` and `to_file` exports it
  unchanged. Import (`claims` in `be-type-text/src/text.rs`) ignores the file
  name and refuses anything that is not UTF-8 or holds a NUL. Readers through
  `TextBlock::text()` decode lossily. The row builder draws each invalid byte
  as its own `U+FFFD`, styled like any text.
- **Undo** lives in the `Document` (`Document::undo` returns cursors), so two
  cores over one document share it.

## Bytes stay intact

- Import a file as text when its name says it is text (`.txt`, `.md` and the
  other text extensions), even if it is not valid UTF-8 or holds a NUL.
- Audit every lossy read (`TextBlock::text()` callers, `copy_utf8`) for one
  that writes back.
- Draw an invalid byte as an error glyph that shows its value (a tinted `FF`),
  through the path that draws invisibles such as `\r`. Add a painting.

## Display transforms

- A per-byte display mode beside `mask` in the row builder, pushing segments
  that map each byte to its display: `Hex` (`68 `), `Ascii` (the character or
  `.`), later `Codepoint` (`U+00E9`). The `leaf` hook cannot do this: it
  restyles a run but cannot change its length without breaking hit-testing.
- The offset column is the hex area's gutter, showing `row * bytes_per_row`.

## Rows the view decides

- Pluggable row breaks: a row every N bytes, the newline byte an ordinary byte.
  Each row sets `wrapped_end`, since the next row starts where it ends.
- Up/Down already go through `VisualRows`. With fixed rows the area can answer
  every row by arithmetic (row `i / N`, column `i % N`), built or not, so hex
  mode never answers `Unknown`: the source-line fallback means nothing there.
- In hex mode `x` is the byte column, not pixels (`VisualRows` lets the view
  pick the unit). The hex and ASCII columns then agree on it, so a mirrored
  cursor keeps its goal column (Two linked editors).
- Home and End by row: add a query for the bytes of the row a caret is in to
  `VisualRows`, and use it in the text area too, where Home and End still go
  by source line.
- Bytes per row is one setting both columns share, chosen from the width
  (16 on a desktop, 8 or fewer on a phone).

## Hex mode

One mode that turns off what assumes a row is a source line, rather than
re-pointing each at byte rows: syntax highlighting, line looks (`line_look`),
and the line commands (duplicate, move and insert line, Tab indenting, Enter's
indent and list continuation, collapsible sections).

## Input

- Overwrite mode in the core (the Insert key), with a block caret; selection,
  Shift+arrows and copy know about it. Useful outside hex.
- The first hex digit is a composition, drawn over the byte it will replace in
  overwrite mode; the second commits the byte. A pending digit makes no undo
  step. Check it against the platform input method on Android and the
  on-screen keyboard.
- Copy offers the bytes as a text fragment and `68 65 6c` as plain text
  (plans/clipboard.md).

## Two linked editors

The hex and ASCII columns are two text areas, each with its own core, over one
`Document`. This replaces drawing the ASCII column as a decoration, which
cannot take clicks, drags, typing or touch handles.

- Linked cursors: the focused editor's cursor set is copied into the other
  after every selection change, undo and redo (cursors are anchors into the
  shared document, so nothing is translated). Compositions are not copied.
  `VerticalMoveStart` is copied too: its x is a byte column in both columns.
- Selection handles work in either column on a phone; dragging one focuses
  that column.
- Typing in the ASCII column inserts or overwrites characters.
- Only the focused editor publishes presence.
- Tab switches columns.
- The scrolls are linked: here rows line up exactly, so the offset is shared.
  Markdown source beside its rendered form would use the same linked cursors
  with a looser link that matches the anchor at the top of each view; keep the
  link general enough for both.

## Order

1. Intact bytes and error glyphs.
2. Home and End by row through `VisualRows`.
3. Display transforms, row breaks and hex mode.
4. Overwrite mode and the pending digit.
5. Linked editors.
6. Replace `hex.rs`, with tests for typing, overwrite, navigation, selection
   and copy, and paintings.

## Open

- Rich text documents: embeds are placeholder bytes and marks sit beside the
  bytes. Read-only in hex, or embeds hidden?
- Pasting plain text in the hex view: parse it as hex, or insert it raw?
- Find: search hex patterns (`de ad`) or text?
- Does a `VirtualList` virtualise against an outer scroll? That decides
  whether the columns share one scroll or link two.
