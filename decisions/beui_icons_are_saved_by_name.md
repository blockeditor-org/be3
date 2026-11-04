# Saved data holds a Lucide icon by name, not by codepoint

Anything saved that refers to an icon stores its Lucide name, looked up with `named` in `beui-lucide`'s `ALL`. A `Glyph` (the icon's codepoint in the font) is used in memory and in the app protocol, never as the saved form.

## Why

Lucide renumbers an icon now and then, so a saved codepoint can point at a different icon after a Lucide update. Names, with the older names kept as aliases in the list, survive updates.

## Rejected

- Saving the codepoint: breaks when Lucide renumbers.
