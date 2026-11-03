# On the HTML core, a plain single-line TextInput is a real `<input>`

Unstyled `TextInput` builds a browser `<input>` (through the platform escape) on the HTML core when
the field is single-line, has no `prose` style and does not `select_on_focus`. Every other field
(`TextArea`, anything rich: marks, syntax colouring, glyphs, completion) and every field natively is
beui's own. New kinds of single-line field are variants of `InputKind`, not new components.

## Why

The browser then edits the field itself: IME, selection, its own context menu, autofill and password
managers, and server-rendered pages whose forms post before and without wasm. Keeping field kinds as
`InputKind` variants keeps one component for one concept, mapping directly onto `<input type>`.

## Rejected

- A separate component per kind of single-line field: parallel forms callers must choose between and
  that must be maintained; a variant of `InputKind` covers it.
- Building `<input>` for fields an `<input>` cannot express: those stay beui's own on both cores.
