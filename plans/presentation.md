# Presentation editor: feature recommendations

Depends on: none.

Recommendations only, not implemented. The deck's model is
`crates/be-type-presentation/src/presentation.rs`, its editor
`crates/be-app-presentation/src` (the slide templates are in
`root/templates.rs`).

Current state: a filmstrip docked beside the stage with drag-reorder,
add-slide (template or link an existing block via the picker), detach/delete,
fullscreen playback with arrow/space/home/end navigation and auto-hiding
controls, and three canvas templates (Title/Regular/Blank). A slide is just
`{id, block}`; no other per-slide metadata exists yet.

## Presenting

- **Speaker notes per slide** — biggest gap. Needs a new field on
  `Slide`. Default to linking a `text` block for the notes, but
  let the user replace it with any block type they choose, same as slides
  themselves are just a block reference.
- **Presenter view** — separate window/second-monitor mode showing current
  slide, next slide thumbnail, notes, and an elapsed timer. The `Stage`
  component in `root/ui.rs` already renders a slide on its own, so this is
  mostly a second viewport reusing it.
- **Jump to slide by number** (type digits + Enter) and **blank/black screen
  toggle** (`B`/`.` like PowerPoint) — cheap additions to `navigate` in
  `root/ui.rs`.
- **Live "follow presenter" mode** — fits this codebase well since presence
  and block sync are already first-class: broadcast the presenter's current
  slide via presence so viewers' clients auto-advance with them. Worth
  prioritizing given how little new infra it needs.
- Auto-advance/timed playback, loop back to slide 1 at the end.

## Slide management

- **Duplicate slide** — currently there's no way to clone a slide's content;
  `Presentation::insert` only links an existing block. This should be a shallow
  copy: clone the `InfiniteCanvas` itself, but any sub-blocks referenced from
  inside it (e.g. an embedded canvas entity pointing at another block) stay
  linked to the same underlying block, with no special deep-copy behavior.
- **Multi-select** (shift/ctrl-click in the filmstrip) for bulk delete/move.
- **Reorder via keyboard** (move up/down) as an alternative to drag-and-drop.
- **Section dividers / grouping** for long decks — filmstrip is a flat list
  today.
- **Grid/overview mode** — a multi-column thumbnail grid instead of the
  single vertical filmstrip, useful once a deck has 20+ slides.

## Content/theming

- **More templates**: two-column, image+caption, section header/agenda.
- **Per-slide transition** (cut/fade) during playback.
- Shared theme / master slide: skipped. Depends on infrastructure work that
  hasn't been done yet (there's no concept of a deck-level style that
  propagates to independently-owned `InfiniteCanvas` slides).

## Export/sharing

- Export to PDF or images, via a dynamic artifact (the pattern the logic
  grid's `dynamic_artifact` module uses to export generated output).
- Print/handout view: skipped.

## Priority

Start with speaker notes, duplicate slide, and follow-presenter mode — they
close the biggest functional gaps and follow-presenter leans directly on
infra that already exists.
