use beui_view::types::icons::{ICON_ARROW_DOWN, ICON_ARROW_UP, ICON_CHEVRONS_DOWN_UP, ICON_COPY_PLUS, ICON_REPLACE, ICON_SEARCH, ICON_SQUARE_DASHED_MOUSE_POINTER};
use beui_view::types::Key;
use beui_view::reactive::*;
use text_editor_core::{EditorCommand, FindDirection, LRDirection, UDDirection};

use super::TextAreaState;

pub(super) fn register(state: TextAreaState, locked: Memo<bool>, find: bool) {
    let editable = create_memo(move || !locked.get());
    if find {
        register_find(state, editable);
    }
    Action::new("text.duplicate-line", "Duplicate the line", move || {
        edit(state, EditorCommand::DuplicateLine(UDDirection::Down))
    })
    .glyph(ICON_COPY_PLUS)
    .shortcut(Chord::ctrl(Key::D).shift())
    .enabled(editable)
    .register();
    Action::new(
        "text.next-occurrence",
        "Select the next occurrence",
        move || edit(state, EditorCommand::DuplicateCursor(LRDirection::Right)),
    )
    .glyph(ICON_SQUARE_DASHED_MOUSE_POINTER)
    .shortcut(Chord::ctrl(Key::D))
    .register();
    Action::new("text.fold", "Fold or unfold the line", move || {
        let command = match state.cursor_line_collapsed() {
            true => EditorCommand::Uncollapse,
            false => EditorCommand::Collapse,
        };
        edit(state, command);
    })
    .glyph(ICON_CHEVRONS_DOWN_UP)
    .shortcut(Chord::ctrl(Key::BracketLeft).shift())
    .register();
}

fn register_find(state: TextAreaState, editable: Memo<bool>) {
    Action::new("text.find", "Find", move || state.open_find(false))
        .glyph(ICON_SEARCH)
        .shortcut(Chord::ctrl(Key::F))
        .register();
    let replace = Action::new("text.replace", "Find and replace", move || {
        state.open_find(true)
    })
    .glyph(ICON_REPLACE)
    .shortcut(Chord::ctrl(Key::H))
    .enabled(editable)
    .register();
    add_to_menu(vec![MenuNode::action(&replace)]);
    Action::new("text.find-next", "Find the next match", move || {
        state.find_step(FindDirection::Next);
    })
    .glyph(ICON_ARROW_DOWN)
    .shortcut(Chord::ctrl(Key::G))
    .register();
    Action::new("text.find-previous", "Find the previous match", move || {
        state.find_step(FindDirection::Previous);
    })
    .glyph(ICON_ARROW_UP)
    .shortcut(Chord::ctrl(Key::G).shift())
    .register();
}

fn edit(state: TextAreaState, command: EditorCommand<'_>) {
    state.execute(command);
    state.set_caret_handle(false);
    state.reveal_cursor();
}
