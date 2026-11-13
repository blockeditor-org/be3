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
- Independent tentative gestures in one block: committing one while another is open would send
  edits ahead of what they were made on, unless one gesture were rebased over the other.
