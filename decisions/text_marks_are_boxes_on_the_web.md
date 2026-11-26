# A text's marks are boxes on the web

On the HTML core, a `Text`'s marks (selections, find hits, peers' selections)
are drawn as boxes in the text's decoration layer (`.beui-mark`), from where a
caret at a mark's start stands to where one at its end does
(`beui-core-html/src/text/web.rs`, `mark_boxes`). While a text is decorated,
its spans' backgrounds are boxes there too (`.beui-span-background`), placed
before the marks.

## Why

- A mark must cover exactly the span between its carets, as natively
  (`RichLayout::selection_rects`). Text area rows lay hidden markup out as
  gaps (a rule's shares, a table's room between cells, a quote's gutter), and
  a mark across them must fill them.
- Both cores then derive a caret and a mark from one mapping, a byte to an x,
  so they cannot disagree.
- Boxes sit under the text in the text's own stacking context. Span
  backgrounds drawn as boxes first keep the native order: backgrounds, marks,
  glyphs.

## Rejected

- **CSS Custom Highlights** (`CSS.highlights`, `::highlight`), which the HTML
  core used before. They paint only glyphs' boxes, never an element's, so a
  mark over a `Gap` (an inline block) left its room bare: a selected rule
  showed a sliver, a selection across a table pipe a gap.
- **Highlights plus boxes for the gaps.** Two ways of drawing one mark, which
  overlap at their edges when translucent (a peer's selection), and still
  two mappings to keep in step.
- **Marks above the text.** A mark's colour is opaque on some themes, so it
  would hide the glyphs, and blending changes its colour from the native one.
