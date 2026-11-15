use std::collections::HashMap;

use be_sdk_type_api::Endpoint;
use be_sdk_type_guest::editor::FromEditor;

use super::*;
use crate::replicas::{Replayed, untaken};

/// A replica that ended took the local edits its last status counts; those it was sent after,
/// the last of its replay, nothing opened after it holds, so the editors that sent them are no
/// longer counted as having them held. A host change among them counts as a local edit but is
/// no editor's.
#[test]
fn a_replica_that_ended_uncounts_only_the_operations_it_never_took() {
    let block = Uuid::new_v4();
    let editor: EditorId = 1;
    let message = |sent: FromEditor| Replayed::Message {
        from: Endpoint::Editor(editor),
        bytes: sent.encode(),
        at: 0,
        delivery: None,
    };
    let replay = [
        message(FromEditor::Operate(vec![1])),
        message(FromEditor::Operate(vec![2])),
        Replayed::Moved {
            head: CommitId::from_hash(be_block_store::Hash::from_bytes([3; 32])),
        },
        message(FromEditor::Gesture(
            be_sdk_type_guest::editor::Gesture::Begin,
        )),
        message(FromEditor::Operate(vec![3])),
    ];
    let mut forwarded = HashMap::from([((block, editor), 6)]);

    // Six local edits were sent and it took four: the gesture and the third operation are past
    // what it took, and only the operation is the editor's to uncount.
    untaken(&mut forwarded, block, (6, 4), &replay);
    assert_eq!(forwarded[&(block, editor)], 5);

    untaken(&mut forwarded, block, (6, 6), &replay);
    assert_eq!(
        forwarded[&(block, editor)],
        5,
        "a replica that took all it was sent uncounts nothing"
    );
}
