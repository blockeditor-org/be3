# Grapheme segmentation

`grapheme-segmentation::is_grapheme_boundary(bytes, index)` implements Unicode
17.0.0 extended grapheme clusters (UAX #29). The index is a byte offset in
`0..=bytes.len()`, including offsets inside a scalar, which return false.
Malformed UTF-8 consumes one byte per U+FFFD, matching text-editor-core's raw-byte
editing behavior. The query allocates nothing and decodes only nearby scalars,
then reads backward as required by GB9c (Indic conjuncts), GB11 (emoji ZWJ
sequences), and GB12/13 (regional-indicator parity). Those rules can require
unbounded context; do not impose a fixed lookbehind limit or truncate at storage
chunk boundaries.

The implementation follows the boundary-query approach in
https://github.com/blockeditor-org/blockeditor/blob/main/packages/unicode_segmentation_2/src/zig_cluster.zig.
The authoritative rules are https://www.unicode.org/reports/tr29/.
Extended_Pictographic is an independent property, not the Emoji property.

`crates/grapheme-segmentation/tools/generate.py` downloads pinned official UCD
properties and the conformance fixture and regenerates `src/tables.rs`. Change
its `VERSION` to upgrade Unicode, run it with Python 3, then run
`./scripts/buck test //crates/grapheme-segmentation:test`. The conformance test
checks every byte offset in every official case, including scalar interiors.
The data is covered by the Unicode license beside the crate manifest.

text-editor-core calls the query directly for cursor stops. Keep that path
independent of line length: do not decode or segment a whole line to answer a
single query.
