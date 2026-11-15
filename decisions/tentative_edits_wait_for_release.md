# Tentative edits wait for release; everything else is written as it happens

An editor writes each change the moment it makes it: text as it is typed, a stroke as it grows.
Grouping a burst for undo (`begin_gesture`, `commit_gesture`, `break_step`) does not delay those
writes.

An edit the person confirms by letting go (dragging an item or a layer, scrubbing a value,
resizing) is tentative. It is applied to the local replica at once, so every view on this device
previews it from the block itself, and shown to peers through presence; it is sent and saved only
when it commits. Cancelling it, a crash, or the instance closing leaves no trace. A tentative edit
only moves or changes what already exists; anything a drop creates is created on commit.

Concurrent tentative edits in one block (two fingers, two controllers, two windows) join one
tentative gesture, which commits as one step when the last of them ends; cancelling one reverts
only its part. Edits in different blocks are independent. Built by plans/tentative_gestures.md.
Colour controls and `NumberInput`'s scrub already work this way inside one editor: they show a
drag through `on_preview` and write `value` once at the end.

This is about block content: what a block is, which peers receive as edits. View state (a scroll
position, the caret, open folders, the shell's dock layout) is the viewer's own and is written on
every change, never held back (guides/blocks/workspaces.md, View state), so a drag whose only
result is view state, such as resizing a split in the shell's dock, writes it as it goes. A tab
dropped in the dock changes the layout only on release because the dock previews the drop from
the place the drag board chose (plans/declarative_dragging.md), not because a write is held.

## Why

A position nobody has let go of is not a decision, so a crash mid-drag must not save it.
Previewing from the block itself keeps derived views (bound lines, z-order, a builder's layout)
right without laying out states that do not exist. Joining concurrent drags means no edit is ever
sent before one it was made on top of. Typed and drawn edits are meant as they are made, so peers
see them at once; saving is batched by the client (at most every 750 ms), so writing often does
not mean saving often.

## Rejected

- Writing every frame of a drag: saves states nobody chose, and fills peers' histories and side
  effects with them.
- Previewing drags only outside the block, drawn by the site: every derived view would have to
  learn about previews, or preview through speculative layout.
- Writing a typed or drawn gesture once, on release: those edits are meant as they are made.
- Holding view state back until release as well: it is not an edit peers receive, and a crash
  that keeps a half-dragged split costs nothing a person cannot drag back.
- Independent tentative gestures in one block: committing one while another is open would send
  edits ahead of what they were made on, unless one gesture were rebased over the other.
