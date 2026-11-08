# An HTML core for beui on the web: what is left

Depends on: none.

beui on the web runs an HTML core (`beui-core-html`), switched in at compile
time by `--cfg beui_html`; guides/beui/core.md (Cores) describes it, and
decisions/beui_has_an_html_core_on_the_web.md says why. What the HTML core
lacks, each with what "done" looks like. Larger directions and the rest of the
ignored props are in plans/deferred/html_core.md.

- **A plain `TextInput`'s caret and selection.** It ignores
  `keyboard_on_focus`, and its `TextInputApi` knows no caret or selection. The
  browser owns the caret, selection and handles
  (decisions/beui_html_text_input_is_a_native_input.md), so map `TextInputApi`
  from the input's `selectionStart`/`selectionEnd` and leave the handles to the
  browser. Done: a test on the HTML core sets and reads the selection through
  `TextInputApi`, and `keyboard_on_focus` is honoured.
- **`Form` posts beui-own fields.** Only plain `TextInput`s carry a `name` into
  the post. Done: a form with a beui-own field (a checkbox, a select) posts its
  value under its name, shown by a server-rendered form test.
- **`Sheet`'s stops as CSS scroll snap.** `Sheet` still drives its `Offset`
  itself on HTML, as natively. Done: on the HTML core a sheet's stops are scroll
  snap points of its scroll element, and a sheet released between stops
  settles on one by the browser's snapping, with no beui animation.
- **Themed scrollbars.** Every `Scroll` on HTML is a browser scroll element
  (unstyled `scroll.rs`,
  decisions/beui_html_scroll_uses_the_element_scroll_position.md), so every
  styled scroll shows the browser's default scrollbar. The unstyled scroll
  already takes `scrollbar: ScrollbarStyle`, which styled fills through
  `scrollbar_style()`: carry the browser bar's thumb and track colours there
  (reactive `Prop`s, so a theme change applies), set through the platform
  escape as `scrollbar-color` and a thin `scrollbar-width`, and have
  `scrollbar_style()` fill them from theme tokens. No second scrollbar prop.
  Done: a painting of a styled scroll on the HTML core shows
  the theme's colours in light and dark.
- **`TreeNode`'s text hooks.** `row_offset`, `texts_within`,
  `set_text_selection` and `set_text_selection_handles` do nothing on HTML
  (`beui-view/src/html/nodes.rs`). Done: each answers on the HTML core as
  natively, with a test per hook.
- **`pastes` and `on_back`.** The HTML core ignores `Interactive`'s `pastes`,
  and `on_back` never fires. Done: a paste event reaches a `pastes` handler
  with its typed clip, and the browser's back (a `popstate`) reaches `on_back`
  before leaving the page, each with a test.
- **The on-screen keyboard.** It shows only where a view places it. Done: on
  touch, a keyboard host opens it for a focused beui-own field, as natively.
  Paste as plain text keeps reading through `navigator.clipboard`; its
  permission prompt is accepted.
- **Delete `crates/beui-font-browser`**, with its entries in the root
  `Cargo.toml`, `buck/cargo/members.bzl` and
  `be-tool-fix-rust-source/src/hash_order.rs`. Nothing uses it: the HTML core
  lays text out in the DOM with its own font faces (`beui-core-html/src/fonts.rs`),
  and the native core on the web shapes with FreeType and HarfBuzz so it
  matches native and its paintings stay deterministic, which measuring through
  the browser would not.
- **No inspector on HTML.** The beui inspector is native only; on HTML the
  browser's own developer tools serve. Add a way to know in advance whether one
  can open (such as `beui::can_open_inspector()`), and have beui-demo disable
  its inspector button where it cannot, removing the `inspector_missing`
  stand-in dialog (`beui-demo/src/lib.rs`). Done: on the HTML core the button
  is disabled. A component tree view may come back later, in beui or as a
  browser extension.
