use beui_text_editor_core::{
    CopyMode, CursorLeftRightStop, EditorCommand, LRDirection, MoveMode, SyntaxNodeDirection,
    UDDirection, VerticalMoveMode,
};

use beui_view::reactive::*;
use beui_view::types::input::apple_keys;
use beui_view::types::{Key, KeyPress};

use super::{Context, TextAreaState};

pub(super) fn key(cx: &Context, press: KeyPress) -> bool {
    if !press.pressed {
        return matches!(press.key, Key::Enter | Key::Space);
    }
    if press.key == Key::Space {
        return true;
    }
    let handled = match cx.single_line {
        true => single_line_command(cx, press),
        false => command(cx, press),
    };
    if handled {
        cx.state.set_caret_handle(false);
        cx.state.reveal_cursor();
    }
    handled
}

pub(super) fn leaves_on_tab(cx: &Context, press: KeyPress) -> bool {
    if !press.pressed {
        return false;
    }
    let escaping = press.key == Key::Escape && !cx.state.find_open().get_untracked();
    let released = cx.state.release_tab(escaping);
    released && press.key == Key::Tab && !press.modifiers.ctrl && !press.modifiers.alt
}

pub(super) fn submit(cx: &Context) {
    cx.state.end_undo_group();
    cx.on_submit.call(());
}

fn selected(state: TextAreaState) -> bool {
    state
        .selection_ranges()
        .iter()
        .any(|range| !range.is_empty())
}

fn single_line_command(cx: &Context, press: KeyPress) -> bool {
    let modifiers = press.modifiers;
    match press.key {
        Key::Enter if cx.on_submit.is_empty() => false,
        Key::Enter => {
            submit(cx);
            true
        }
        Key::Tab | Key::Escape => false,
        Key::C | Key::X if modifiers.ctrl && !selected(cx.state) => true,
        Key::ArrowUp | Key::ArrowDown if modifiers.alt => false,
        Key::B | Key::I | Key::F | Key::H | Key::G | Key::D | Key::BracketLeft
            if modifiers.ctrl =>
        {
            false
        }
        _ => command(cx, press),
    }
}

fn command(cx: &Context, press: KeyPress) -> bool {
    let state = cx.state;
    let modifiers = press.modifiers;
    let command = modifiers.ctrl;
    match press.key {
        Key::Escape if state.find_open().get_untracked() => {
            state.close_find();
            return true;
        }
        Key::C | Key::X if command && cx.masked.get_untracked() => return true,
        Key::C if command => {
            state.copy_to_clipboard(CopyMode::Copy);
            return true;
        }
        Key::X if command => {
            state.copy_to_clipboard(match cx.locked.get_untracked() {
                true => CopyMode::Copy,
                false => CopyMode::Cut,
            });
            return true;
        }
        _ => {}
    }

    if let Some(caret) = caret_key(press, apple_keys()) {
        let mode = match modifiers.shift {
            true => MoveMode::Select,
            false => MoveMode::Move,
        };
        match caret {
            CaretKey::Across(direction, stop) => {
                state.execute(EditorCommand::MoveCursorLeftRight {
                    mode,
                    direction,
                    stop,
                })
            }
            CaretKey::Row(direction) => state.move_by_row(
                direction,
                match modifiers.shift {
                    true => VerticalMoveMode::Select,
                    false => VerticalMoveMode::Move,
                },
            ),
            CaretKey::Duplicate(direction) => {
                state.move_by_row(direction, VerticalMoveMode::Duplicate)
            }
            CaretKey::SyntaxNode(direction) => {
                state.execute(EditorCommand::SelectSyntaxNode(direction))
            }
        }
        return true;
    }

    if cx.locked.get_untracked() {
        return match press.key {
            Key::A if command => {
                state.execute(EditorCommand::SelectAll);
                true
            }
            _ => false,
        };
    }

    match press.key {
        Key::Backspace | Key::Delete => state.execute(EditorCommand::Delete {
            direction: match press.key {
                Key::Backspace => LRDirection::Left,
                _ => LRDirection::Right,
            },
            stop: delete_stop(press, apple_keys()),
        }),
        Key::Enter if command => state.execute(EditorCommand::InsertLine(match modifiers.shift {
            true => UDDirection::Up,
            false => UDDirection::Down,
        })),
        Key::Enter => state.execute(EditorCommand::Newline),
        Key::Tab => state.execute(EditorCommand::IndentSelection(match modifiers.shift {
            true => LRDirection::Left,
            false => LRDirection::Right,
        })),
        Key::A if command => state.execute(EditorCommand::SelectAll),
        Key::Z if command => state.execute(match modifiers.shift {
            true => EditorCommand::Redo,
            false => EditorCommand::Undo,
        }),
        Key::Y if command => state.execute(EditorCommand::Redo),
        _ if command && !modifiers.alt => return shortcut(state, press.key),
        _ => return false,
    }
    true
}

/// What an arrow, Home or End key does to the carets; Shift selects instead of moving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaretKey {
    Across(LRDirection, CursorLeftRightStop),
    Row(UDDirection),
    Duplicate(UDDirection),
    SyntaxNode(SyntaxNodeDirection),
}

/// The caret key `press` is, on Apple's systems (`apple`, where Command is held as `ctrl`)
/// or elsewhere. Home and End go to the line's ends, and with Ctrl to the document's,
/// everywhere; Apple's Cmd+Left and Cmd+Right go to the line's ends and Cmd+Up and
/// Cmd+Down to the document's, where elsewhere Ctrl+Left and Ctrl+Right go by word and
/// Ctrl+Up and Ctrl+Down by row.
pub fn caret_key(press: KeyPress, apple: bool) -> Option<CaretKey> {
    let modifiers = press.modifiers;
    let across = |direction| {
        let stop = match press.key {
            Key::Home | Key::End if modifiers.ctrl => CursorLeftRightStop::Document,
            Key::Home | Key::End => CursorLeftRightStop::Line,
            _ if apple && modifiers.ctrl => CursorLeftRightStop::Line,
            _ if modifiers.alt || modifiers.ctrl => CursorLeftRightStop::Word,
            _ => CursorLeftRightStop::UnicodeGraphemeCluster,
        };
        Some(CaretKey::Across(direction, stop))
    };
    let vertical = |direction: UDDirection| {
        Some(if modifiers.alt && modifiers.ctrl {
            CaretKey::Duplicate(direction)
        } else if apple && modifiers.ctrl {
            CaretKey::Across(
                match direction {
                    UDDirection::Up => LRDirection::Left,
                    UDDirection::Down => LRDirection::Right,
                },
                CursorLeftRightStop::Document,
            )
        } else if modifiers.alt {
            CaretKey::SyntaxNode(match direction {
                UDDirection::Up => SyntaxNodeDirection::Parent,
                UDDirection::Down => SyntaxNodeDirection::Child,
            })
        } else {
            CaretKey::Row(direction)
        })
    };
    match press.key {
        Key::ArrowLeft | Key::Home => across(LRDirection::Left),
        Key::ArrowRight | Key::End => across(LRDirection::Right),
        Key::ArrowUp => vertical(UDDirection::Up),
        Key::ArrowDown => vertical(UDDirection::Down),
        _ => None,
    }
}

/// How far Backspace or Delete reaches, on Apple's systems (`apple`) or elsewhere:
/// Alt, and Ctrl elsewhere, take a word; Apple's Cmd takes the rest of the line.
pub fn delete_stop(press: KeyPress, apple: bool) -> CursorLeftRightStop {
    let modifiers = press.modifiers;
    match () {
        _ if apple && modifiers.ctrl => CursorLeftRightStop::Line,
        _ if modifiers.alt || modifiers.ctrl => CursorLeftRightStop::Word,
        _ => CursorLeftRightStop::UnicodeGraphemeCluster,
    }
}

fn shortcut(state: TextAreaState, key: Key) -> bool {
    let Some(letter) = letter(key) else {
        return false;
    };
    let Some(selected) = state.selected().get_untracked() else {
        return false;
    };
    let schema = state.schema();
    let Some(tool) = schema
        .tools()
        .iter()
        .find(|tool| tool.shortcut == Some(letter))
    else {
        return false;
    };
    state.execute((tool.command)(&selected));
    true
}

fn letter(key: Key) -> Option<char> {
    const LETTERS: [(Key, char); 26] = [
        (Key::A, 'a'),
        (Key::B, 'b'),
        (Key::C, 'c'),
        (Key::D, 'd'),
        (Key::E, 'e'),
        (Key::F, 'f'),
        (Key::G, 'g'),
        (Key::H, 'h'),
        (Key::I, 'i'),
        (Key::J, 'j'),
        (Key::K, 'k'),
        (Key::L, 'l'),
        (Key::M, 'm'),
        (Key::N, 'n'),
        (Key::O, 'o'),
        (Key::P, 'p'),
        (Key::Q, 'q'),
        (Key::R, 'r'),
        (Key::S, 's'),
        (Key::T, 't'),
        (Key::U, 'u'),
        (Key::V, 'v'),
        (Key::W, 'w'),
        (Key::X, 'x'),
        (Key::Y, 'y'),
        (Key::Z, 'z'),
    ];
    LETTERS
        .iter()
        .find(|(held, _)| *held == key)
        .map(|(_, letter)| *letter)
}
