use beui_core::icons::{
    ICON_ARROW_DOWNWARD, ICON_ARROW_UPWARD, ICON_CONTENT_COPY, ICON_FIND_REPLACE, ICON_SEARCH,
    ICON_SELECT_ALL, ICON_UNFOLD_LESS,
};
use beui_core::input::Key;
use beui_view::reactive::{Action, Chord, Memo, clone, create_memo};
use text_editor_core::{EditorCommand, FindDirection, LRDirection, UDDirection};

use super::TextAreaState;

pub(super) fn register(state: &TextAreaState, disabled: &Memo<bool>) {
    let editable = create_memo(clone!(disabled -> move || !disabled.get()));
    Action::new(
        "text.find",
        "Find",
        clone!(state -> move || state.open_find(false)),
    )
    .glyph(ICON_SEARCH)
    .shortcut(Chord::ctrl(Key::F))
    .register();
    Action::new(
        "text.replace",
        "Find and replace",
        clone!(state -> move || state.open_find(true)),
    )
    .glyph(ICON_FIND_REPLACE)
    .shortcut(Chord::ctrl(Key::H))
    .enabled(editable.clone())
    .in_menu()
    .register();
    Action::new(
        "text.find-next",
        "Find the next match",
        clone!(state -> move || {
            state.find_step(FindDirection::Next);
        }),
    )
    .glyph(ICON_ARROW_DOWNWARD)
    .shortcut(Chord::ctrl(Key::G))
    .register();
    Action::new(
        "text.find-previous",
        "Find the previous match",
        clone!(state -> move || {
            state.find_step(FindDirection::Previous);
        }),
    )
    .glyph(ICON_ARROW_UPWARD)
    .shortcut(Chord::ctrl(Key::G).shift())
    .register();
    Action::new(
        "text.duplicate-line",
        "Duplicate the line",
        clone!(state -> move || edit(&state, EditorCommand::DuplicateLine(UDDirection::Down))),
    )
    .glyph(ICON_CONTENT_COPY)
    .shortcut(Chord::ctrl(Key::D).shift())
    .enabled(editable)
    .register();
    Action::new(
        "text.next-occurrence",
        "Select the next occurrence",
        clone!(state -> move || edit(&state, EditorCommand::DuplicateCursor(LRDirection::Right))),
    )
    .glyph(ICON_SELECT_ALL)
    .shortcut(Chord::ctrl(Key::D))
    .register();
    Action::new(
        "text.fold",
        "Fold or unfold the line",
        clone!(state -> move || {
            let command = match state.cursor_line_collapsed() {
                true => EditorCommand::Uncollapse,
                false => EditorCommand::Collapse,
            };
            edit(&state, command);
        }),
    )
    .glyph(ICON_UNFOLD_LESS)
    .shortcut(Chord::ctrl(Key::BracketLeft).shift())
    .register();
}

fn edit(state: &TextAreaState, command: EditorCommand<'_>) {
    state.execute(command);
    state.set_caret_handle(false);
    state.reveal_cursor();
}
