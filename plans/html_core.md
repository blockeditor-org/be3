# An HTML core for beui on the web: what is left

Depends on: none.

beui on the web runs an HTML core (`beui-core-html`), switched in at compile time by `--cfg beui_html`; guides/beui/core.md (Cores) describes it, and decisions/beui_has_an_html_core_on_the_web.md says why. Features the native core has and the HTML core lacks or ignores:

- Browser-native text editing for `TextArea` and the `TextInput`s that are
  not plain (EditContext or `contenteditable`), a switch in unstyled.
- A plain `TextInput` on HTML ignores `keyboard_on_focus`, and its
  `TextInputApi` knows no caret, selection or handles (the browser's).
- `Form` cannot post a beui-own field: only plain `TextInput`s carry a
  `name` into the post.
- A `Drawing` with no `size` draws nothing in a server-rendered page, since
  the HTML core measures nothing there; so the scrollbar thumb and the colour
  picker's areas and sliders appear only once the page runs.
- `Sheet`'s stops as CSS scroll snap: `Sheet` still drives its `Offset`
  itself on HTML, as natively.
- The browser's scrollbar on a styled `Scroll` is the browser's default look
  (following `color-scheme`), not the theme's: `scrollbar-color` from the
  theme, and `SCROLLBAR_WIDTH`, are not passed down.
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
- `beui-font-browser` is still unused.
- The inspector: it reads the native `Document`, so beui-demo's inspector
  button shows a stand-in on HTML (`beui::open_inspector()` answers false).
- The on-screen keyboard on HTML shows only where a view places it; no keyboard
  host opens it for a focused field, and Paste as plain text reads the
  clipboard through `navigator.clipboard`, which asks for permission.
- HTML text uses the browser's font metrics, which round a font's ascent and
  descent to the nearest pixel where FreeType rounds them outward, so a line
  can sit a pixel off native (a 20px title, icons).
- `TextAreaState::lay_out_prose` and the `TextArea` rows still lay prose out through `Piece` and `RichOptions` from `beui_view::types` rather than through `measure_text` alone (the infinite canvas's export calls it too), so text layout is only partly behind the text geometry contract.
