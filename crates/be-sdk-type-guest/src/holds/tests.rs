use super::*;
use crate::editor::{FromEditor, Gesture};

/// `editor`'s gesture `id`.
fn gesture_in(holds: &mut Holds, editor: EditorId, id: u64, gesture: Gesture) -> Took {
    holds.message(editor, Some(&FromEditor::Gesture { id, gesture }))
}

/// An edit in `editor`'s gesture `id`.
fn operate_in(holds: &mut Holds, editor: EditorId, id: u64) -> Took {
    holds.message(
        editor,
        Some(&FromEditor::Operate {
            gesture: Some(id),
            operation: vec![1],
        }),
    )
}

/// `editor`'s one gesture.
fn gesture(holds: &mut Holds, editor: EditorId, gesture: Gesture) -> Took {
    gesture_in(holds, editor, 0, gesture)
}

/// An edit in `editor`'s one gesture.
fn operate(holds: &mut Holds, editor: EditorId) -> Took {
    operate_in(holds, editor, 0)
}

fn named(editor: EditorId, id: u64) -> GestureId {
    GestureId {
        editor: Some(editor),
        id,
    }
}

fn serial(took: &Took) -> u64 {
    match took.this {
        This::Held(serial) => serial,
        this => panic!("held, not {this:?}"),
    }
}

mod a_begin_commits_an_undo_cancels_and_leaving_cancels_its_own;
mod held_operations_count_when_the_layer_commits;
mod one_editors_two_gestures_end_on_their_own;
