use super::*;
use crate::{LiveEdit, Root};
use be_block_model::{Change, Count, Document, Edit, List, Model};

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Rows {
    rows: List<Row>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
struct Row {
    text: String,
    note: String,
    count: Count,
}

impl Root for Rows {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x726f_7773_2d61_6273_6f72_622d_7465_7374);
}

fn set(row: be_block_model::ObjectId, text: &str) -> Change {
    Row::TEXT.set(row, &text.to_owned())
}

/// Absorbs `edits` one after another, as a burst of submissions or a held layer told whole is,
/// and checks the absorbed edit leaves what applying them in turn does.
fn absorbed(start: &Document<Rows>, edits: Vec<Change>) -> Edit {
    let mut edits = edits.into_iter().map(Edit::from);
    let mut absorbed = edits.next().unwrap();
    let mut each = start.clone();
    each.apply(&absorbed);
    for edit in edits {
        each.apply(&edit);
        assert!(Document::<Rows>::absorb_operation(&mut absorbed, edit).is_none());
    }
    let mut whole = start.clone();
    whole.apply(&absorbed);
    assert_eq!(whole.root(), each.root());
    absorbed
}

/// A later set of a field drops an earlier one only when nothing else in the edit reads the
/// field or its object whole: a removal that compares every field, a conditional set or an
/// add still sees the earlier value, so the absorbed edit leaves what the edits did one by one.
#[test]
fn a_set_absorbed_drops_an_earlier_set_only_when_nothing_else_reads_it() {
    let start = Document::new(&Rows {
        rows: [Row::default()].into_iter().collect(),
    });
    let row = start.root().rows[0].id;
    let holds_x = |edit: &Edit| edit.0.contains(&set(row, "X"));

    // A drag's frames: the last set of each field is all that is left.
    let frames = absorbed(
        &start,
        vec![
            set(row, "X"),
            Row::NOTE.set(row, &"n".to_owned()),
            set(row, "Y"),
        ],
    );
    assert!(!holds_x(&frames));
    assert_eq!(frames.0.len(), 2);

    // Removing the row if blank sees X, so the row is kept.
    let removed = absorbed(
        &start,
        vec![
            set(row, "X"),
            Change::remove_if_blank::<Row>(row),
            set(row, "Y"),
        ],
    );
    assert!(holds_x(&removed));
    let mut kept = start.clone();
    kept.apply(&removed);
    assert_eq!(kept.root().rows[0].text, "Y");

    // A conditional set expects X.
    let Change::Set { value, .. } = set(row, "X") else {
        unreachable!()
    };
    let Change::Set { value: z, .. } = set(row, "Z") else {
        unreachable!()
    };
    let conditional = absorbed(
        &start,
        vec![
            set(row, "X"),
            Change::SetIf {
                object: row,
                field: Row::TEXT.index(),
                expected: value,
                value: z,
            },
            set(row, "Y"),
        ],
    );
    assert!(holds_x(&conditional));

    // An add to the field reads what an earlier set left.
    let count = |value: u8| Change::Set {
        object: row,
        field: Row::COUNT.index(),
        value: vec![value],
    };
    let added = absorbed(&start, vec![count(5), Row::COUNT.add(row, 1), count(6)]);
    assert!(added.0.contains(&count(5)));
}
