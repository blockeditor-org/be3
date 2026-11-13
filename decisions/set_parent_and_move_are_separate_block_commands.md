# `set-parent` and `move` are separate block commands

`block-command` keeps both `set-parent(block-location)` and `move(move-command)`
(with `reorder` and `place` beside them).

## Why

They do different things. `move` transfers a block between two containers that
list it in their content: be-host opens both containers' editors, takes the
block out of the source's content, puts it in the destination's, and sets the
graph parent only when it moved the block rather than a link (`is-reference`).
`reorder` and `place` likewise edit a container's content. `set-parent` changes
only the graph parent, of a block the app already holds in content of its own
(a canvas or map taking in a dropped block, a database adopting its view), and
the runtime checks and applies it without any container editor.

## Rejected

- Expressing `set-parent` as `move`: `move` needs a source and destination
  container editor and edits their content, which the apps that set a parent
  have already done themselves.
- Expressing `move` as `set-parent`: the containers' content would no longer
  list what the graph says they hold.
