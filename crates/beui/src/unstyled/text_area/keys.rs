use text_editor_core::{
    CopyMode, CursorHorizontalPositionMetric, CursorLeftRightStop, EditorCommand, FindDirection,
    LRDirection, MarkdownCommand, MoveMode, SyntaxNodeDirection, UDDirection, VerticalMoveMode,
};

use crate::input::{Key, KeyPress};
use crate::reactive::copy_text;

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

pub(super) fn submit(cx: &Context) {
    cx.state.external_edit();
    cx.on_submit.call();
}

fn copy(state: &TextAreaState, mode: CopyMode) {
    let text = state.copy(mode);
    if !text.is_empty() {
        copy_text(text);
    }
}

fn single_line_command(cx: &Context, press: KeyPress) -> bool {
    let modifiers = press.modifiers;
    match press.key {
        Key::Enter => {
            submit(cx);
            true
        }
        Key::Tab | Key::Escape => false,
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
    let state = &cx.state;
    let modifiers = press.modifiers;
    let command = modifiers.ctrl;
    match press.key {
        Key::Escape if state.find_open().get_untracked() => {
            state.close_find();
            return true;
        }
        Key::C | Key::X if command && cx.masked.get_untracked() => return true,
        Key::C if command => {
            copy(state, CopyMode::Copy);
            return true;
        }
        Key::X if command => {
            copy(state, CopyMode::Cut);
            return true;
        }
        _ => {}
    }

    let direction = match press.key {
        Key::ArrowLeft | Key::Home => Some(LRDirection::Left),
        Key::ArrowRight | Key::End => Some(LRDirection::Right),
        _ => None,
    };
    if let Some(direction) = direction {
        state.execute(EditorCommand::MoveCursorLeftRight {
            mode: match modifiers.shift {
                true => MoveMode::Select,
                false => MoveMode::Move,
            },
            direction,
            stop: if matches!(press.key, Key::Home | Key::End) {
                CursorLeftRightStop::Line
            } else if modifiers.alt || modifiers.ctrl {
                CursorLeftRightStop::Word
            } else {
                CursorLeftRightStop::UnicodeGraphemeCluster
            },
        });
        return true;
    }

    if matches!(press.key, Key::ArrowUp | Key::ArrowDown) {
        let direction = match press.key {
            Key::ArrowUp => UDDirection::Up,
            _ => UDDirection::Down,
        };
        if modifiers.alt && modifiers.ctrl {
            state.execute(EditorCommand::MoveCursorUpDown {
                direction,
                mode: VerticalMoveMode::Duplicate,
                metric: CursorHorizontalPositionMetric::Byte,
                stop: CursorLeftRightStop::UnicodeGraphemeCluster,
            });
        } else if modifiers.alt {
            state.execute(EditorCommand::SelectSyntaxNode(
                match direction == UDDirection::Up {
                    true => SyntaxNodeDirection::Parent,
                    false => SyntaxNodeDirection::Child,
                },
            ));
        } else {
            state.execute(EditorCommand::MoveCursorUpDown {
                direction,
                mode: match modifiers.shift {
                    true => VerticalMoveMode::Select,
                    false => VerticalMoveMode::Move,
                },
                metric: CursorHorizontalPositionMetric::Byte,
                stop: CursorLeftRightStop::UnicodeGraphemeCluster,
            });
        }
        return true;
    }

    match press.key {
        Key::Backspace | Key::Delete => state.execute(EditorCommand::Delete {
            direction: match press.key {
                Key::Backspace => LRDirection::Left,
                _ => LRDirection::Right,
            },
            stop: if modifiers.alt || modifiers.ctrl {
                CursorLeftRightStop::Word
            } else {
                CursorLeftRightStop::UnicodeGraphemeCluster
            },
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
        Key::B if command => {
            state.execute(EditorCommand::Markdown(MarkdownCommand::Bold));
        }
        Key::I if command => {
            state.execute(EditorCommand::Markdown(MarkdownCommand::Italic));
        }
        Key::Z if command => state.execute(match modifiers.shift {
            true => EditorCommand::Redo,
            false => EditorCommand::Undo,
        }),
        Key::Y if command => state.execute(EditorCommand::Redo),
        Key::F if command => state.open_find(false),
        Key::H if command => state.open_find(true),
        Key::G if command => {
            state.find_step(match modifiers.shift {
                true => FindDirection::Previous,
                false => FindDirection::Next,
            });
        }
        Key::D if command && modifiers.shift => {
            state.execute(EditorCommand::DuplicateLine(UDDirection::Down));
        }
        Key::D if command => {
            state.execute(EditorCommand::DuplicateCursor(LRDirection::Right));
        }
        Key::BracketLeft if command && modifiers.shift => {
            state.execute(match state.cursor_line_collapsed() {
                true => EditorCommand::Uncollapse,
                false => EditorCommand::Collapse,
            });
        }
        _ => return false,
    }
    true
}
