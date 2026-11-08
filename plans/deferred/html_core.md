# The HTML core: larger directions

Depends on: none.

Set aside until the user picks them up.

- Browser-native text editing for `TextArea` and the `TextInput`s that are
  not plain (EditContext or `contenteditable`), a switch in unstyled.
- Porting be-host itself to the HTML core; it runs the native core with wgpu
  on the web.
- Web apps as iframes, which could run the HTML core.
- The rest of the props the HTML core ignores: `Interactive`'s `key_active`,
  `claims_touch` and most of the forwarding group (`inhibits_shortcuts`,
  `clipboard_keys`, `forwards_wheel`; `on_forward` hears only the pointer, no
  keys, focus or hover); `Overlay`'s `window` and `shown_in`; `Root`'s
  `system_bars`; `Canvas`'s `scrolls_across`; `VirtualList`'s `text`;
  `Selectable`'s colours, handles and toolbar; and `use_keyboard()`, always
  true.
- `Break`'s copy test across Firefox and WebKit
  (decisions/beui_break_copies_without_a_newline.md): the shared scripts run
  only in Chromium, and only Chromium is installed.
