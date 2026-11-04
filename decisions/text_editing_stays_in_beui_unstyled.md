# Text editing stays in beui's unstyled layer on both cores

`TextArea` and the `TextInput`s that are not plain single-line fields keep their own editing (`beui_text_editor_core`, rows, `VirtualList`, completion and handles) on both cores. On the HTML core they rely on base `Text`'s declarative props (`marks` as the CSS Custom Highlight API, `glyphs` as replacing `<span>`s, `carets` as positioned elements, `hang` as a negative `text-indent`), text geometry answered by the browser, and text input and IME from a hidden `<textarea>`. Read-only display text is selected and copied by the browser; inside editors `TextArea` owns selection on both cores.

## Why

- One editor for both cores, with real DOM text, so find-in-page, translation and screen readers see editors' content.
- Line breaking and shaping come from the browser on the HTML core; the editor only asks for geometry and never computes it.
