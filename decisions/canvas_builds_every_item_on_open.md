# The canvas builds an item for every shape when it opens

Opening the infinite canvas builds an item for every shape in the document, not only for
those near the view.

## Why

Building items lazily would need each item's reach (stroke, hand-drawn wobble, labels and
text) before the item exists, and would move the cost into the first pans over a dense
canvas. It would at best halve opening 10,000 shapes, since loading the document and the
test host's store take as long as building the items.

## Rejected

- Building items only once they come near the view: for the reasons above.
