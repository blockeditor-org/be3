use std::collections::HashMap;

use be_sdk_type_api::Endpoint;
use be_sdk_type_guest::editor::FromEditor;

use super::*;
use crate::replicas::{Replayed, acknowledged};

/// A replica reopened under an editor holds every operation the editor sent, less those the
/// host still keeps to give it again: what it tells the editor its snapshot holds, in the
/// editor's own count, so the editor knows which of the operations it was told landed come
/// again.
#[test]
fn a_reopened_replica_holds_an_editors_operations_less_those_replayed() {
    let block = Uuid::new_v4();
    let (editor, other): (EditorId, EditorId) = (1, 2);
    let forwarded = HashMap::from([((block, editor), 5), ((block, other), 2)]);
    let message = |from: EditorId, sent: FromEditor| Replayed::Message {
        from: Endpoint::Editor(from),
        bytes: sent.encode(),
        at: 0,
        delivery: None,
    };
    let replay = [
        message(editor, FromEditor::Operate(vec![1])),
        message(other, FromEditor::Operate(vec![2])),
        message(
            editor,
            FromEditor::Gesture(be_sdk_type_guest::editor::Gesture::Begin),
        ),
        message(editor, FromEditor::Operate(vec![3])),
    ];
    assert_eq!(
        acknowledged(&forwarded, block, &replay, editor),
        3,
        "two of the editor's five operations are given again; a gesture is not an operation"
    );
    assert_eq!(acknowledged(&forwarded, block, &replay, other), 1);
    assert_eq!(
        acknowledged(&forwarded, Uuid::new_v4(), &replay, editor),
        0,
        "an editor new to a block was told of none"
    );
}
