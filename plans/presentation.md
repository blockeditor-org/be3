# Presentation editor: speaker notes, duplicating and following

Depends on: none.

Not implemented. The deck's model is
`crates/be-type-presentation/src/presentation.rs`, its editor
`crates/be-app-presentation/src` (the slide templates are in
`root/templates.rs`).

Current state: a filmstrip docked beside the stage with drag-reorder,
add-slide (template or link an existing block via the picker), detach/delete,
fullscreen playback with arrow/space/home/end navigation and auto-hiding
controls, and three canvas templates (Title/Regular/Blank). A slide is just
`{id, block}`; no other per-slide metadata exists yet.

## Work

- **Speaker notes per slide** — biggest gap. Needs a new field on
  `Slide`. Default to linking a `text` block for the notes, but
  let the user replace it with any block type they choose, same as slides
  themselves are just a block reference.
- **Duplicate slide** — currently there's no way to clone a slide's content;
  `Presentation::insert` only links an existing block. This should be a shallow
  copy: clone the `InfiniteCanvas` itself, but any sub-blocks referenced from
  inside it (e.g. an embedded canvas entity pointing at another block) stay
  linked to the same underlying block, with no special deep-copy behavior.
- **Live "follow presenter" mode** — fits this codebase well since presence
  and block sync are already first-class: broadcast the presenter's current
  slide via presence so viewers' clients auto-advance with them. Worth
  prioritizing given how little new infra it needs.
