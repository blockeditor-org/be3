# A move between drop sites is the app's: the receiving site's commit gets the payload

When a carry lands on a different drop site than the one it came from (a card dragged from
one column to another), the receiving site's `commit` gets the payload and the app does the
whole move: it takes the item out of its home and puts it at the chosen place, in one edit
of its own data. The source hears only that its carry was dropped, as it does for any drop.

This covers sites over one app's data, such as two lists in one block. A move between
blocks is not one edit of one app's data: there the source holds a tentative removal and
the target writes the insert first (plans/declarative_dragging.md, step 5).

## Why

The two lists are views of the app's data, and a move is one change to that data. Doing it
in one place keeps it one edit, so undo, sync and the reactive rebuild see a move rather than
an insert and a removal that could land apart or with only one of them. The payload already
names the item, so the receiving side has all it needs, and the drag board stays free of any
idea of homes.

## Rejected

- **A `moved_away` callback on `Carry`:** the source would commit its removal as a separate
  tentative edit when the drop lands. Two edits from two sites for one move, with an ordering
  to get right, and a second way to learn that the drop happened.
- **A payload that carries its own removal** (a closure the receiver calls): the same split,
  hidden in the payload, and payloads stop being plain data.
