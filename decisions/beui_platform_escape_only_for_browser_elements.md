# The platform escape is only for components whose HTML form is the browser's own element

`beui::escape::html_escape` exists only on the HTML core (it panics natively). It is called under
`if CORE == Core::Html`, in unstyled or styled, for a component whose HTML form is the browser's own
element (a field, a form, a link, a scroller), with a beui version in the other branch keeping the
same public API.

## Why

Browser elements bring behaviour beui cannot reproduce (native editing, posting forms and following
links before wasm loads). Keeping the escape to those components keeps the public API identical on
both cores and keeps the cores free of any knowledge of fields, forms, links or scrolling.

## Rejected

- Switching on `CORE` to work around a primitive one core lacks: the primitive belongs in the core
  instead.
- Teaching the cores about fields, forms, links or scrolling: the components that need browser
  elements own that, through the escape.
