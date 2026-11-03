# A dock tab adds only a menu of actions to the dock's bars, never a node

A tab contributes to the dock's bars (tiled tab bars, sidebars, windows and the stacked bar) only
through `dock_menu(items)`, a `Memo<Vec<MenuNode>>`. Its most-used actions are pinned into the bar
with `.pin(Pin::Icon | Pin::Label | Pin::Title)`; everything else stays behind More. The bar itself
is the dock's.

## Why

A menu that is a list of actions rather than a node lets a dock that lays out tabs built elsewhere
(another app, across the host) draw it from rows it was sent, using `ActionBuilder::detached`
actions, and pass the pick back. Pinning puts a tab's few most-used controls in reach without giving
the dock a node, and leaves the rest of the tab's page for its own toolbar.

## Rejected

- Letting a tab put its own nodes in the dock's bar: a dock laying out tabs built elsewhere could not
  draw them from rows it was sent.
