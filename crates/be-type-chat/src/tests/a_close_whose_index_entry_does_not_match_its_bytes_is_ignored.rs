use be_block_model::Change;

use crate::Closing;

use super::*;

#[test]
fn a_close_whose_index_entry_does_not_match_its_bytes_is_ignored() {
    let chat = long(120);
    let crate::ChatOp::Close(closing) = chat.closing(&[]).expect("closes") else {
        panic!("a close");
    };
    let mut ok = chat.clone();
    ok.apply(&crate::ChatOp::Close(closing.clone()));
    assert_eq!(ok.segments().len(), 1);

    // The bytes of a different segment under the same edit.
    let mut other = closing.bytes.clone();
    let last = other.len() - 1;
    other[last] ^= 1;
    let mut forged = chat.clone();
    forged.apply(&crate::ChatOp::Close(Closing {
        edit: closing.edit.clone(),
        bytes: other,
    }));
    assert!(forged.segments().is_empty(), "the hash does not match");

    // An edit that adds no index entry, or one that removes more than the segment holds.
    let changes: Vec<Change> = closing
        .edit
        .0
        .iter()
        .filter(|change| !matches!(change, Change::Series { .. }))
        .cloned()
        .collect();
    let mut bare = chat.clone();
    bare.apply(&crate::ChatOp::Close(Closing {
        edit: changes.into_iter().collect(),
        bytes: closing.bytes.clone(),
    }));
    assert!(bare.segments().is_empty());
    assert_eq!(bare.messages(), chat.messages());
}
