# A drag is a carry choosing among the places it can reach

Every drag in beui that moves something is a carry on the tree's drag board. `Interactive`'s
`drag` lifts a `Carry` with a payload, every drop site that accepts the payload
(`use_drop_site`) gives the places it could land as a `DragSpec`, and each point chooses the
closest of them all (`closest`, `free`, `custom`, `when_far`, `snap_radius`, `thrown`, …). A
site draws what is chosen in it from its `over`, and only the release commits it. The board
does the floating, Escape, keyboard carries, edge scrolling, several pointers at once and
what the inspector shows. The old API (`Draggable`, `DropTarget`, the board's `Target` and
`DragSource`) is gone; drags that move no item (a pan, a text selection, a scrub) stay on
`on_drag`. Adapted from *Declarative Dragging* (Horowitz, Blinn, Heer, UIST 2026); the steps
are in plans/declarative_dragging.md.

## Why

Before, each site wrote its own geometry: the dock's `resolve`, its tool bars' bands and
thresholds, `Reorder`'s hit logic, the game board's press state machine, each app's hand-made
drop targets. They disagreed on thresholds, could not be reached from the keyboard, and each
hand-coded Escape, edge scrolling and previews or went without. Naming the places instead
gives every drag those for free and makes the choosing testable without a core: a spec is
pure (`evaluate`), tested in `beui-types`, and the board on a fake `Space`.

- **Places come from layout that exists.** A list's slots are read from its rows' laid-out
  rects (`slots_in`), the dock's from `layout_tree` on the state after a drop (a pure
  function), a timeline's from its model. One pass at the lift.
- **Nothing is written before release** (decisions/tentative_edits_wait_for_release.md). A
  site previews from `over`, or, inside the block the drag began in, as a tentative edit; a
  value control's thumb previews by writing and writes back its start on a put-down.
- **Choosing and resting are separate.** Nearest rest by default; a zone where the rest is far
  from the pointer (a pane's edge band, `nearest_edge`).
- **Places are named by neighbours** (`After(Option<K>)`), so a peer's insert does not move
  them.
- **Keys reach the same places.** A focused source lifts with Space, Move in the palette or
  Ctrl+Shift and an arrow; the arrows step between places and a live region names each.

## Rejected

- **Building speculative states to read where an item would rest** (Dragology renders each
  reachable state): building components for a state that does not exist opens scopes and runs
  effects, which may write a block.
- **Blending view trees between states** (`between`): nothing blends between discrete states;
  the dock's parting springs each pane's rect instead.
- **A numeric optimiser** (`vary`): every continuous drag in the repo is the pointer minus the
  grab, clamped or stepped, which `free` is.
- **A drag mode that writes as it goes**: a crash or Escape would leave half a move written.
- **Implicit context arming glides** (rows animating because a drag is under way somewhere):
  a row glides when its own index changes (`Glide on=`), whatever moved it.
- **Keeping `Draggable` and `DropTarget` beside the board** as the simple case: two ways to
  drag, with a site on one unable to take an item from the other.
