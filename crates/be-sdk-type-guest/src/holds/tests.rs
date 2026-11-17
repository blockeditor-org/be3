use super::*;
use crate::editor::{FromEditor, Gesture};

fn gesture(holds: &mut Holds, editor: EditorId, gesture: Gesture) -> Took {
    holds.message(editor, Some(&FromEditor::Gesture(gesture)))
}

fn operate(holds: &mut Holds, editor: EditorId) -> Took {
    holds.message(editor, Some(&FromEditor::Operate(vec![1])))
}

fn serial(took: &Took) -> u64 {
    match took.this {
        This::Held(serial) => serial,
        this => panic!("held, not {this:?}"),
    }
}

mod a_begin_commits_an_undo_cancels_and_leaving_cancels_its_own;
mod held_operations_count_when_the_layer_commits;
