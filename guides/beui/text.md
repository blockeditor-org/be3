# Beui text

Text in beui: the base `Text` node, styled text, fonts and icons, the text area editor and rich text, and selecting read-only text. The component layers around them are in [components.md](components.md).

## The base `Text`

A base `Text` takes concrete values (a `FontId`, a colour) and knows nothing of the theme. Its children are its content, all laid out as one paragraph so wrapping, the caret and selection cross style changes:

- strings and `Prop<String>`s in the Text's own style;
- `Span`s, which override parts of the style over a run (computed runs are a `ForEach` of `Span`s);
- `Icon`s, one glyph from the icon font that holds no bytes, so the caret steps over it and copy skips it: `<Text><Icon glyph=ICON_SAVE />"Save"</Text>` copies as "Save". An icon's gap to its label is layout, not a space a selection could take;
- `TextItem`s, a child node laid out inline as one character (U+FFFC), or with `at` placed at a byte index like a caret;
- `Gap`s and `Break`s, room and line ends holding no bytes, so a soft-wrapped terminal row copies as one line.

`overflow` wraps at words by default; `Ellipsis` cuts each line; `Visible` lays lines out unbroken for something that scrolls or clips. There is no clip mode: cut text is `Visible` inside something that clips. `on_truncated` reports whether ellipsised text is cut, so a component can show it whole another way (a tooltip). `WrapAt::Anywhere` in a monospace font holds exactly `cols` characters a line; measure the advance as `cursor_pos(_, 1).x`, since a galley's `size()` is rounded up.

`marks` (selections), `glyphs` (centred markers such as a text area's whitespace marks) and `carets` are byte ranges over the joined content and only repaint when they change; so do colour and decoration changes. Shaping is cached per span. Bold and italic are synthesised by FreeType, not separate faces. A wrapping text's `min_width` is its longest unbreakable run, so a row never squeezes it into a column of letters.

`measure_text(node)` answers index-to-point and point-to-index from the node's last layout. Inside a `Drawing`, `painter.lay_out_text` returns a `TextBlock` that `painter.draw_text` draws, so what is measured is what is drawn; the same `TextBlock` comes from `lay_out_text` outside one.

## Layout scale and raster

Two scales, like pinch-zoom and Ctrl+/- in a browser:

- **Layout scale** (`Context::pixels_per_point`, platform scale times zoom): text is laid out and everything snaps to the pixel grid at this scale, and galleys are cached by it. Changing it lays the document out again.
- **Raster** (`PainterState::raster`): how much finer a subtree is drawn (a hosted window's scale, a zoomed canvas's `Zoom::raster`). Only painting reads it: under a raster other than 1, FreeType rasterises glyphs unhinted at the scaled size at their laid-out positions, so a zoomed canvas is sharp without relayout. The browser's backend ignores it.

## Styled text

`styled::Text`, `Span` and `Icon` take the base children and resolve tokens into a font and colour: `size` (a `TextSize`), `weight`, `family`, `italic`, `color` (a `TextColor` role resolved through the theme, `Custom` only for colours that are data), `style` (a `TextStyle` preset whose props a given prop overrides) and `overflow`. Text wraps by default, so a one-line label in a button, row or bar takes `overflow=Overflow::Ellipsis`. `TITLE` and `HEADING` are accessibility headings on both cores.

An icon beside a label is one `Text`, not a row of two; the icon sizes itself from the text:

```rust
view! {
    <Text style=TextStyle::CAPTION>
        <Icon glyph=ICON_CLOCK />
        "Edited five minutes ago"
    </Text>
}
```

`TextStyleProvider` is context, so it reaches what is *created* inside it, which is not always what is laid out there: text a render callback builds for an overlay takes the provider around where the callback was written. A `Span` or `Icon` inside a `Text` takes what it does not set from that `Text`. Raw pixel sizes and `Color32`s are for base `Text` drawn rather than designed (canvas labels, a terminal, a game board).

## Fonts and icons

`crates/beui-fonts` holds the faces (Inter, Hack for monospace, Noto Emoji as fallback, Lucide for icons): their bytes, which beui-font-freetype loads, and `FACES`, from which the HTML core writes `@font-face` rules. Its BUCK exports each file for web bundles.

Icons are Lucide's, in `crates/beui-lucide` (re-exported as `beui_core::icons`; it knows nothing of beui, so the block catalog uses it too). An icon is a `Glyph` that only the list can make, so an icon prop cannot be handed text. Anything saved holds an icon by name, since Lucide renumbers codepoints ([decision](../../decisions/beui_icons_are_saved_by_name.md)). To find an icon for a purpose, search the tags in `crates/beui-lucide/src/list.rs` ("duplicate" finds `ICON_COPY_PLUS`) rather than guessing from names. The list is generated; to update Lucide, unpack the package (`npm pack lucide-static@VERSION`, `tar xzf`) and run `./scripts/buck run //crates/be-tool-local:bin -- icons PACKAGE` from inside the repository.

## Text areas

`unstyled::TextArea` is the multiline editor, built from ordinary nodes: a row per document line in a `VirtualList`, each a `Text` of the line's spans, with a gutter and list markers, voids and embedded widgets as `TextItem`s. It owns a `beui_text_editor_core::Core` through a `TextAreaState` the caller holds. Inline and block `TextWidget`s the caller names are how a block editor embeds a block in the text and drives the document from its own toolbar.

- Nothing polls the document: code that changes it from behind (adopting a remote edit) calls `TextAreaState::sync()`; `end_undo_group()` also ends the undo group, which blur and submit do, and which a document whose own history cannot span an outside edit needs (a host-grouped `LiveText` must not: a peer's edit would cut its steps).
- `TextInput` is the area's single-line mode over a buffer it owns, so a fix to editing lands in both (except the HTML core's plain `<input>`, [components.md](components.md#inputs-and-forms)).
- `read_only` keeps the area focusable and selectable but refuses every edit; `disabled` removes it from focus and input; `find=false` lets Ctrl+F reach the surrounding program.
- Text is measured only from the tree: the area reports its size through `on_content_size`, and a caller sizes a box from that (the canvas grows a text entity while it is edited). It takes its view size from its own layout, so one inside a scaled `CanvasItem` is not laid out again on pan or zoom. The one layout away from the tree is the canvas's export, `TextAreaState::lay_out_prose`, which hands lines to a layout function the caller passes so the area names no core.
- A focused area whose view gets shorter (a phone's keyboard or a bar coming up under it) scrolls a caret that was in sight back into sight, so the row holding the caret stays built and the input method keeps being told the caret's rectangle rather than the whole area's. A caret already scrolled away stays where it is.
- `styled::TextArea` takes tokens (`size`, `color`, `padding`) and has no scale prop: text drawn zoomed sits in a scaled space.
- Up and Down move by the rows the view shows, not by source line, so a wrapped line is crossed a row at a time. The core asks the view through `VisualRows` (`beui-text-editor-core/src/core/rows.rs`), which `CursorHorizontalPositionMetric::Screen` carries: where across its row a caret sits, and the row before or after it with the caret stop nearest a goal x the core keeps across repeated moves. The view decides what a row is (a wrapped row here; a hex view's fixed-width chunk would do too), skips rows it hides, and flags a row whose end is the next row's start so the caret settles a stop before it. The area answers from its laid-out rows (`Surface::row_step`) through the `TextAreaLayout` it publishes, and `TextAreaState::move_by_row` is the call; a caret whose row or neighbour is not built (far from the viewport) moves by source line instead, as `Byte` does. Home and End still go by source line.

### Looks

How the area draws is three render hooks keyed by the line description (`LineLook`) markdown and rich text both give, as in Slate: `leaf` turns a styled run into a `Span` (so every run stays in the line's one `Text`), `marker` draws a list marker or void inside a `TextItem`, and `line` wraps a line (quote bars, code block backgrounds, indent). They come in a required `TextAreaLook` with `TextAreaColors` and the `TextAreaGeometry` of every absolute size; unstyled keeps only proportions of the text size. `styled::text_area_look()` and its siblings build them for a theme, and a caller drawing voids handles `MarkerKind::Void` and passes every other marker to `DocumentMarker`.

The engine says what is on a line, never how big: markers are measured by their `Text` during layout, and wrapped lines hang after them. In markdown and plain text whitespace is real text, and its markers are the row's `glyphs`, so selecting never moves text.

### Rich text

A `TextLanguage::RichText` document is structured as in Slate: the bytes are the text alone, every line is a block, and formatting is a set of `Marks` beside the bytes, each setting a key over inline text or whole lines. A `Schema` (`beui-text-editor-core/src/schema.rs`; `schema::basic` has the usual marks and blocks) gives keys their meaning: what Enter, Backspace and Tab do to a block, the tools, each leaf's style. `Core` owns only generic operations (`SetMark`, `SetBlock`, `InsertVoid`, `Transform`). Copy and paste carry text without marks. `styled::FormatBar` shows a schema's tools.

Marks (`be-lib-sequence/src/marks.rs`) are anchored to sequence positions and converge in any order, so copies exchanging them agree; the demo's Rich text page sends them over a simulated network. Undo removes an edit's marks rather than marking old values over them, so a concurrent mark from another copy survives.

## Selecting text

`Selectable` (`styled::SelectableText` over it) is each core's own read-only selection: the browser's on HTML, and `beui-view/src/native/selectable.rs` natively, which selects across the `Text`s under it and across `VirtualList` rows, built or not. A selection end in a list is a key, a text index and a byte index, never a node, so a recycled row cannot carry it to another key. Select All and copy work without building rows; copy reads unbuilt rows from the list's `text` prop, and a list without one copies them as empty lines. `selected_text` reads the document, so it works only from a handler or inside `with_reactive_scope`.
