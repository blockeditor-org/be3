# An HTML core for beui on the web: what is left

Depends on: none.

beui on the web runs an HTML core (`beui-core-html`), switched in at compile time by `--cfg beui_html`; guides/beui/core.md (Cores) describes it, and decisions/beui_has_an_html_core_on_the_web.md says why. Features the native core has and the HTML core lacks or ignores:

- A plain `TextInput` on HTML ignores `keyboard_on_focus`, and its
  `TextInputApi` knows no caret, selection or handles (the browser's).
- `Form` cannot post a beui-own field: only plain `TextInput`s carry a
  `name` into the post.
- `Sheet`'s stops as CSS scroll snap: `Sheet` still drives its `Offset`
  itself on HTML, as natively.
- Every `Scroll` on HTML is a browser scroll element (unstyled `scroll.rs`,
  decisions/beui_html_scroll_uses_the_element_scroll_position.md), so every
  styled scroll shows the browser's default scrollbar (following
  `color-scheme`), not the theme's. Pass the theme's thumb and track colours
  as `scrollbar-color` and a thin `scrollbar-width` down from the styled
  `scrollbar_style`, so a styled scroll looks themed on the web.
- `TreeNode`'s `row_offset`, `texts_within`, `set_text_selection` and
  `set_text_selection_handles` do nothing on HTML.
- Props the HTML core ignores: `Interactive`'s `pastes`, `key_active`,
  `claims_touch` and most of the forwarding group (`inhibits_shortcuts`,
  `clipboard_keys`, `forwards_wheel`; `on_forward` hears only the pointer, no
  keys, focus or hover); `Overlay`'s `window` and `shown_in`;
  `Root`'s `system_bars`; `Canvas`'s `scrolls_across`; `VirtualList`'s `text`;
  `Selectable`'s colours, handles and toolbar. `on_back` never fires and
  `use_keyboard()` is always true.
- `Break`'s copy test across Firefox and WebKit (decisions/beui_break_copies_without_a_newline.md): the shared
  scripts run only in Chromium.
- `color_var!` is meant to be the only way to make a `ColorVar`, but
  `ColorVar::__declared_at`, which it expands to, is public (doc-hidden), so
  any code can name one by string.
- Delete `crates/beui-font-browser` (and its entry in
  `be-tool-fix-rust-source/src/hash_order.rs`). Nothing uses it: the HTML core
  lays text out in the DOM with its own font faces (`beui-core-html/src/fonts.rs`),
  and the native core on the web shapes with FreeType and HarfBuzz so it
  matches native and its paintings stay deterministic, which measuring through
  the browser would not.
- The beui inspector is native only; on HTML the browser's own developer tools
  serve. beui-demo shows its inspector button only where
  `beui::open_inspector()` can open one, instead of a stand-in.
- The on-screen keyboard on HTML shows only where a view places it; no keyboard
  host opens it for a focused field, and Paste as plain text reads the
  clipboard through `navigator.clipboard`, which asks for permission.
- `TextAreaState::lay_out_prose` and the `TextArea` rows still lay prose out through `Piece` and `RichOptions` from `beui_view::types` rather than through `measure_text` alone (the infinite canvas's export calls it too), so text layout is only partly behind the text geometry contract.
