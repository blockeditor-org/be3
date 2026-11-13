# A beui field on the web uses the browser's keyboard

On the web, a focused beui-own field (a `TextInput`, a `TextArea`) types through the browser's own
on-screen keyboard, opened by focusing the page's hidden IME agent, on both cores: the HTML core's
`ime.rs` agent and `beui-platform-web`'s textarea. beui's own keyboard (`beui::native::keyboard`)
never opens by itself there: the web platform reports that it shows a keyboard
(`PlatformServices::shows_keyboard`), so the runner never enables beui's, and the HTML core has no
keyboard host. A view may still place beui's keyboard itself (beui-demo's Inputs page does).

## Why

The browser's keyboard is the one people know on their phone: their language, layout, suggestions,
dictation and swipe typing, which beui's keyboard does not have. The IME agent already carries
composition and paste to the field, and the viewport gives up the keyboard's room
(`interactive-widget=resizes-content`). Paste as plain text keeps reading through
`navigator.clipboard`, and its permission prompt is accepted.

## Rejected

- Porting beui's keyboard host to the HTML core and opening it for a focused field, as natively: a
  second keyboard competing with the browser's, which opens anyway for the agent on a touch screen.
