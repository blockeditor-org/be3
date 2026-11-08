# Presentation editor: later features

Depends on: plans/presentation.md (the presenter view needs speaker notes).

Recommendations only, set aside until the user picks them. The deck's model is
`crates/be-type-presentation/src/presentation.rs`, its editor
`crates/be-app-presentation/src`.

## Presenting

- **Live "follow presenter" mode**: broadcast the presenter's current slide
  through presence so viewers' clients advance with them. Open questions for
  the user: who counts as the presenter (whoever has `editor.presenting()`
  on?); whether following is opt-in (a "Follow NAME" button) or always on,
  which would move the slide under anyone editing the deck; two presenters at
  once; whether followers go full screen; what presence turned off
  (`presence(false)`) does; and names, since peers are keyed by client id
  (`peers::<P>() -> Vec<(u64, P)>`). Testable headless with
  `BeuiTest::set_peers` and `take_shown_presence`.
- **Presenter view** — separate window/second-monitor mode showing current
  slide, next slide thumbnail, notes, and an elapsed timer. The `Stage`
  component in `root/ui.rs` already renders a slide on its own, so this is
  mostly a second viewport reusing it.
- **Jump to slide by number** (type digits + Enter) and **blank/black screen
  toggle** (`B`/`.` like PowerPoint) — cheap additions to `navigate` in
  `root/ui.rs`.
- Auto-advance/timed playback, loop back to slide 1 at the end.

## Slide management

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
