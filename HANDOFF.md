# Handoff: the text format bar as a dock bar, and keyboard gifts into joined editors

Never land this file. Branch `wip/text-format-dock-bar`, on top of `land/dock-part-2-fixes`
(the fixes that stand on their own: revision preview, phone layout and group title for joined
editors, and the stacked bar's title and count).

## The task

The user asked for be-app-text's format bar to be a beui docking-WM tool bar (a `DockBar`, as the
infinite canvas and the desktop use) that does not auto-hide. Commit "wip: text format bar as a
dock bar" does that: `TextEditor` (crates/be-app-text/src/root.rs) is now a `Docking` with one
locked bare pane (tab `text`) and a `DockBar` `format` beside its bottom edge
(`DockBarPlacement::Beside(Bottom, Start)`), shown while `format_bar_shown` (chrome shown, not
hex view, and Markdown or touch). `FormatBar` (root/toolbar.rs) is a `DockBarSurface` with a grip,
Done first on touch, then the Markdown formats. `State::typing` is gone. The be-app-text tests and
paintings were updated on the earlier branch; on this one the paintings still need accepting
(`./scripts/test root::tests --update` in crates/be-app-text; it matches other crates too).

## Why it is not landed

Giving the text editor a `Docking` makes its dock join its placer's (every workspace tab, a deck's
notes, a canvas card), and that exposed bugs in joined docks that every joined editor (the canvas
too) has. Fixed and landing separately on `land/dock-part-2-fixes`:

1. A revision preview was hidden by the live child's joined panes (be-sdk-shell panel.rs).
2. A joined editor did not know it was on a phone, so Rename opened as a card (be-sdk-beui
   `phone_layout`).
3. A spliced child's group lost its tab's title (WIT `dock-group.title`).
4. The stacked bar named a joined editor's stage tab ("Text") and counted no open tabs
   (`DockState::standing_in`).

Still open, on this branch:

5. Keyboard gifts into joined children (typing right after "Add notes" was lost).
6. Back on a phone from a document opened from the files returned to the document.

## What the gift rework does (commit "wip: keyboard gifts go to the view holding ...")

crates/be-host/src/compositor/app_view.rs, `focus_when_given` and `keyboard_follows_content`:

- A docked child's content shows in one of its dock's tab views (`ViewPlacement::Dock`), built
  where the placer's tab shows, not among the placer's children, so it had no `ChildFocus`.
  `docked_focus` gives such a view an `AskedFocus` from the placement the host keeps
  (`be_host_runtime::docked_placer`, from `docks::placed_by`).
- `takes`: of a child's views, the one holding its content
  (`be_host_runtime::content_view(app, instance) == region`) takes the gift and reports the child
  up. `content_view` defaults to MAIN, so a docked child whose content stays in its main view
  (the file tree side panel) still takes it there.
- A joined text shows its content in its main view first, then in its dock tab view once its dock
  is hosted, and the main view goes. If the main view took the gift (`GIFTED_MAIN`), the dock tab
  view that then holds the content focuses itself while nothing has the focus.

## What fails, and what was seen

Host tests (crates/be-host-tests), each `./scripts/test NAME`:

- `typing_right_after_adding_notes_reaches_the_notes`
- `ctrl_tab_walks_the_workspace_tabs_while_an_editor_has_the_keyboard`
- `ctrl_tab_leaves_the_workspace_tabs_alone_while_the_host_palette_is_open`
- `on_a_phone_back_leaves_each_document_opened_from_the_files`

All four pass on main, where the text editor does not join. Driven by hand with
`./scripts/buck run -c be3.apps=text //crates/be-host:dev -- --fresh --pin-clock` and the same
`drive` steps (the failing test prints them), the notes and Ctrl+Tab flows work: typing after
"Add notes" (5 of 5 runs), clicking and typing into the second of two open texts. In the host
tests, though, the final tree of the two Ctrl+Tab tests has the second document's tab selected and
no joined "Text" pane at all: nothing is placed in `#workspace.block`, nothing crashed, and the
log is empty. Find what differs between `:dev` and the host-test run first.

The phone Back test failed earlier because the file tree (also docked, content in its main view)
stopped taking its gift under a rule that left every docked main view out; `takes` above replaced
that rule, but the test has not passed since.

## A trap

Calling `holds_focus(catcher)` on an AppView catcher in be-host's compositor (to watch whether a
main view lost the keyboard) made a later click into a joined dock tab view not take the focus, with
one text open fine and two broken. Removing it fixed that. Something about focus watching in
be-host's document for app view catchers deserves a look before relying on it.

## Earlier attempts that did not work

- A nested `DockHost` around the text's `Docking` to keep it from joining: under a docked editor
  frame a `DockHost` passes through to the frame's slot, so it still joins.
- An opt-out from joining was rejected by the coordinator (a parallel way to dock).
- Waiting in the main view while the instance is docked lost the gift when the main view was torn
  down; following the keyboard when the main view lost focus "to nothing" marked too broadly and
  broke clicks with two texts open.
