# Middle-click paste off keeps the primary selection empty, with its protocols still advertised

While the input settings' "Middle-click paste" is off, the Linux compositor still advertises
`zwp_primary_selection_device_manager_v1` and the data-control protocols' primary selection, and
drops every request that would set the primary selection, so it stays empty. It does not cancel
the source that asked.

## Why

- A program that finds no primary selection protocol emulates one: Firefox then pastes the
  clipboard on a middle click.
- A program whose source is cancelled has lost its selection, and a terminal then clears the text
  it just highlighted.

## Rejected

- Not advertising the globals while the setting is off: programs emulate middle-click paste.
- Cancelling the source of a primary selection: terminals clear their highlight.
