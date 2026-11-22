# Fonts on skrifa and harfrust

Depends on: none.

Replace the C font stack under beui's native core, HarfBuzz (`harfbuzz_rs`) for shaping and
FreeType for everything else, with Rust crates: harfrust shapes, skrifa reads faces, hints and
gives outlines, colour layers and bitmap strikes, zeno fills outlines into coverage masks, and
`png` decodes the PNG strikes of colour emoji fonts. Layout, line breaking, the galley and the
glyph cache stay beui's own (guides/beui/text.md, Wrapping); only what is behind `FontBackend`
(`beui-core/src/font.rs`) changes. The HTML core keeps the browser's text, but its `Drawing`
measurer moves from ttf-parser to skrifa, so the repository reads fonts one way.

## Why

- No C in the beui library: the freetype-sys overlay build script, the HarfBuzz C++ compile, and
  the libpng and zlib built beside FreeType all go. libc++ and setjmp stay in base for PDFium and
  libghostty-vt.
- harfrust is HarfBuzz ported to Rust by HarfBuzz's own organisation and tested against its
  suite; skrifa is the font reader Chrome uses in place of FreeType for web fonts, its TrueType
  interpreter and autohinter written to match FreeType's output. Both read through read-fonts,
  so one `FontRef` serves shaping and drawing.
- PDFium's FreeType no longer shares symbol names with beui's (guides/app_libraries.md).

## What each crate does

- **harfrust** shapes a run. It neither hints nor scales: its positions are in font units. Scale
  each advance and offset to 26.6 and round it there, as HarfBuzz's scaled font does, so
  positions, and with them subpixel phases, match HarfBuzz's exactly. Keep one owned
  `ShaperData` per face, and pass the pixel size as the point size so `trak` applies as it does
  with `set_ppem` today. Script runs stay ours; take the script from `icu_properties` (already
  in the tree through be-lib-graphemes) and drop `unicode-script`.
- **skrifa** gives the cmap (fallback coverage, and `system.rs`'s scan in place of
  `FT_New_Face`), ascender and descender, glyph bounds, outlines, `COLR` layers and bitmap
  strikes.
  - Hinting: at layout scale, outlines come from a `HintingInstance` per face and pixel size,
    cached as `RasterFace::size` caches FreeType's; options match FreeType's defaults
    (`Engine::AutoFallback`: the font's own instructions, the autohinter for a font without
    them; smooth, normal mode). Under a raster other than 1, the unhinted outline is used, as
    `FT_LOAD_NO_HINTING` does today.
  - Metrics: FreeType reads hhea's ascender and descender and grid-fits them (ceil and floor at
    the pixel size). skrifa's `Metrics` prefers OS/2's typographic values when
    `USE_TYPO_METRICS` is set, so read hhea and round as FreeType does, or every baseline moves.
  - Bounds: `GlyphMetrics::bounds` covers `glyf` faces; for CFF faces (Noto CJK and other
    system fallbacks) take the bounds of the outline.
  - Colour: `COLR` v0 layers drawn in their palette colours, as FreeType's `FT_LOAD_COLOR` does
    today; a v1 glyph is drawn as its plain outline, also as today. Bitmap strikes (CBDT, sbix)
    are chosen and scaled as `color.rs` does now, PNG data decoded with `png`.
- **zeno** fills an outline (a skrifa `OutlinePen` writing zeno path commands) into an 8-bit
  mask, with the subpixel shift and the raster scale as its transform. Its rasteriser is a cell
  rasteriser in FreeType's style, so overlapping contours (common in variable fonts) are
  covered exactly.
- **Synthetic bold and italic**, which skrifa does not have, are ours:
  - italic is the shear `FT_GlyphSlot_Oblique` applies, x += 0.2126 · y (0x0366A in 16.16),
    put into zeno's transform;
  - bold is `FT_Outline_EmboldenXY` at `FT_GlyphSlot_Embolden`'s strength (ppem / 24 pixels),
    written from its description rather than translated from FreeType's C, whose licence would
    then want a credit: each contour's points move out along the bisector of their two edges
    by half the strength over (1 + the cosine of the angle between them), capped by the edge
    lengths at sharp turns, and the whole outline moves up and right by half the strength, so
    the left side bearing and baseline stay. Orientation decides out: a counter shrinks.
  - Both apply to the hinted outline at the layout size, before the raster's scale, which is
    FreeType's order today. Fallback faces need them even if beui later bundles real bold and
    italic faces.

## Steps

1. **harfrust shapes; FreeType still draws.** Replace `harfbuzz_rs` in `shape_line`, and its
   `extents` with skrifa's bounds. Script runs move to `icu_properties`. Glyphs and positions
   should not change, so every painting should stay as it is: one that changes is a parity bug
   (the 26.6 rounding, the point size, a script boundary) to fix, not to accept.
2. **skrifa, zeno and png draw.** Replace FreeType in the rasteriser, metrics, sizes, the cmap
   and `system.rs`, with the synthetic styles above. Delete the `freetype-sys` fixup and
   overlay (`third-party/rust/fixups.bzl`, `third-party/rust/overlays/freetype-sys`),
   `third-party/libpng` and `third-party/zlib`, and run `./scripts/buckify`. Paintings change by
   anti-aliasing and hinting detail: look at a sample side by side with today's before accepting,
   bold and italic ones included, which should keep their weight and slant. Test the emboldening
   on its own: a square grows by exactly the strength, a counter shrinks, a spike stays bounded.
   Compare the size of the beui library module before and after, and put it in the pull request.
3. **Name and documents.** Rename `beui-font-freetype` to `beui-font-skrifa` and `FreetypeFonts`
   to `SkrifaFonts`; a backend is named after the technology it is built on, so another (a
   DirectWrite one, say) can sit beside it. Move beui-core-html's `text_block/faces.rs` to
   skrifa. Then update what names the old stack: guides/beui/text.md, guides/app_libraries.md,
   decisions/app_libraries_are_coarse.md, decisions/guests_are_single_threaded.md,
   decisions/a_root_links_beui_as_an_app_does.md (its first reason goes; the one triple for
   every guest remains), decisions/app_tests_run_as_wasm_guests.md, the comments in
   `crates/be-sdk-library-base/BUCK`, `buck/toolchains/BUCK` and `third-party/system/BUCK`, and
   plans/deferred/i18n.md (What exists, Shaping). Record the choice and what was
   rejected below in decisions/, and delete this plan.

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
