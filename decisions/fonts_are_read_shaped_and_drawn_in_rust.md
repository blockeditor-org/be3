# Fonts are read, shaped and drawn in Rust: skrifa, harfrust and zeno

beui's native core (`crates/beui-font-skrifa`, its `FontBackend`) shapes with harfrust, reads
faces with skrifa (the cmap, metrics, bounds, hinted and unhinted outlines, COLR v0 layers and
bitmap strikes), fills outlines into coverage masks with zeno and decodes PNG strikes with `png`.
Synthetic bold and italic are beui's own: FreeType's oblique shear, and an emboldening that moves
each point out along its edges' bisector (crates/beui-font-skrifa's `outline.rs`). Layout, line
breaking, the galley and the glyph cache stay beui's (guides/beui/text.md). The HTML core keeps
the browser's text, and measures a `Drawing`'s with skrifa too, so the repository reads fonts one
way.

## Why

- No C in the beui library: FreeType and HarfBuzz were C and C++ compiled for every platform and
  for wasm, with libpng and zlib beside FreeType, a build-script overlay and a libc to link
  against. PDFium's FreeType no longer shares symbol names with beui's.
- harfrust is HarfBuzz ported to Rust by HarfBuzz's own organisation and tested against its
  suite; skrifa is the font reader Chrome uses in place of FreeType for web fonts, its TrueType
  interpreter and autohinter written to match FreeType's output. Both read through read-fonts, so
  one `FontRef` serves shaping and drawing.
- Positions stay HarfBuzz's: harfrust scales to 26.6 as HarfBuzz's scaled font does (rounding;
  the HarfBuzz beui bundled before truncated, so advances moved by up to 1/64 pixel). Hinting
  keeps FreeType's defaults (the font's instructions, the autohinter for a font without them,
  smooth normal mode), and the ascender and descender are hhea's fitted to the grid as FreeType
  fits them, so baselines stay where they were.

## Rejected

- **parley** for layout: it chooses line breaks itself, at the last opportunity that fits, and
  cannot take beui's tiered break zone, the range of widths over which a line keeps its layout,
  or resuming from the line before an edit. It also brings fontique's fallback.
- **fontique** for system fonts, for now: its fallback is by script (fontconfig is asked with a
  sample of each script's characters), where beui looks a font up per character, which emoji and
  symbols in the Common script need; and beui has nothing that picks a system family by name.
  Reconsider it when a host on macOS, Windows or Android needs that system's own fallback, or
  when beui lists system fonts.
- **vello_cpu (with glifo)** to draw glyphs: a second 2D renderer inside beui to fill masks. It
  makes sense only as part of moving beui's whole renderer to Vello.
- **swash**: its own font reader, in maintenance; its authors' work moved to skrifa and harfrust.
  **rustybuzz** is superseded by harfrust. **ttf-parser** and **ab_glyph** do not hint.
- **A rasteriser of our own**: an accumulation rasteriser (font-rs, fontdue) clamps the sum of
  signed areas, which is slightly wrong at the anti-aliased edges of overlapping contours, and
  zeno is small enough that owning it gains little. Glyphs are filled on the CPU into an atlas
  either way; anti-aliased paths that change every frame (the canvas's lines) belong on the GPU
  in beui's renderer, a separate problem.
- **Keeping HarfBuzz's old truncation** of scaled positions: current HarfBuzz rounds, and
  emulating the old rounding would need shaping at a finer scale and still differ wherever a
  kerning pair adds to an advance.
