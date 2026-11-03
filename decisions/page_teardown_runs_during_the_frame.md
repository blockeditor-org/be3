# A closed page is disposed during the frame that closes it

beui disposes a closed page's scopes and nodes in the frame that closes it, rather than
deferring the teardown until after the frame.

## Why

Teardown should be fast rather than hidden. Deferring it works around its cost instead of
removing it.

## Rejected

- Disposing a closed page after the frame: it saved about 12% of opening a page, but only
  by moving the teardown out of the measurement. It is kept in reserve for when nothing
  better is left.
