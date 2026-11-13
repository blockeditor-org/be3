# BeuiTest keeps its `take_*` wrappers for shell messages

`BeuiTest` keeps one-line wrappers such as `take_child_focus`,
`take_dialog_closes` and `take_picked`, each picking one `ShellMessage`
variant through `take_shell_messages(pick)`. Only aliases that add nothing
went: `take_copies` (no callers) and `take_copied_images`, which was
`take_copied(PNG)`.

## Why

- A test reads as what the editor asked the host for
  (`test.take_child_focus()`), not as a match on the protocol's enum.
- Replacing a wrapper puts a four-line closure at every call site (3 to 16 of
  them each) and has each test import `ShellMessage` and know its payload's
  shape, so a protocol change touches every test instead of one wrapper.
- `take_shell_messages(pick)` stays the one general way; the wrappers are
  names for its common picks, not a second mechanism.

## Rejected

- Removing the wrappers in favour of `take_shell_messages(pick)` at every call
  site (plans/api_simplification.md §15, written out for the child focus and
  dialog tests): longer tests with no fewer concepts.
