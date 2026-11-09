# Presentation editor: duplicating slides and speaker notes

Depends on: none.

The deck's model is `crates/be-type-presentation/src/presentation.rs`, its
editor `crates/be-app-presentation/src` (the slide templates are in
`root/templates.rs`). Follow-presenter mode and other ideas are in
plans/deferred/presentation.md.

Current state: a filmstrip docked beside the stage with drag-reorder,
add-slide (template or link an existing block via the picker), "Detach slide"
(removes the entry and keeps the block), and full-screen playback with
arrow/space/home/end navigation and controls shown on hover. A slide is
`Slide { block: Option<Uuid> }` in a `List`; the list object is its id, and
`block: None` shows as "Broken link". No code in be-app-presentation registers
an `Action` today; registering Present, Next and Add slide as actions would also
list them in the host's command palette (guides/apps/editor.md, Actions and the palette), and is
optional here.

## 1. Duplicate slide

- An `Action` "Duplicate slide" on Ctrl+D (Cmd+D on macOS), also in the tile's
  menu, copies the slide's block with `host().copy_blocks(source, parent)` (the
  block and everything inside it, for any block type) into the deck, inserts
  the copy after the current slide, and selects it.
- Done: a test duplicates a canvas slide and a text slide, the copy is a new
  block after the original and selected, and editing the copy leaves the
  original unchanged. A painting of the filmstrip after duplicating.

## 2. Speaker notes

- `Slide` gains `notes: Option<Uuid>`, listed in `references()` (so the notes
  block is held, not reclaimed) and handled in `child_edit`: `Delete` clears
  it, `Replace` swaps it.
- The notes block is made on first use, not per slide. With a notes block, the
  notes pane shows its text editor as a child; without one, it shows an "Add
  notes" button that creates an empty text block by type id (`create_with`,
  no be-type-text dependency) inside the deck, links it and moves the focus
  into the child editor, so typing starts there and no keystroke is lost. Any block may replace it, as a slide's block may be any
  type.
- The notes pane sits under the stage in the deck's dock; on a phone it is a
  pane of the stacked dock, as the filmstrip is. Notes stay hidden while
  presenting until a presenter view exists (deferred).
- Done: a test presses "Add notes" on a slide with none (one text block is
  made and linked, and the focus is in its editor), deletes it (the link clears), and checks no notes block exists for
  untouched slides. Paintings of the notes pane on desktop and phone.

## Order of work

1. Duplicate slide.
2. Speaker notes.
