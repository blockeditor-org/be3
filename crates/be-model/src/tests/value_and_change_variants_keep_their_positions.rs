use std::collections::BTreeMap;

use super::*;
use crate::{Bounds, Cells, Object, Place, Stamped, Value};

fn tag<T: serde::Serialize>(value: &T) -> u8 {
    postcard::to_stdvec(value).unwrap()[0]
}

#[test]
fn value_and_change_variants_keep_their_positions() {
    let values = [
        Value::Register(Vec::new()),
        Value::Count(0),
        Value::List(Vec::new()),
        Value::Map(BTreeMap::new()),
        Value::Grid(Cells::default()),
        Value::Latest(BTreeMap::new()),
    ];
    for (position, value) in values.iter().enumerate() {
        assert_eq!(
            usize::from(tag(value)),
            position,
            "stored documents hold {value:?} at this position; add new variants at the end"
        );
    }

    let object = ObjectId::ROOT;
    let place = Place { object, field: 0 };
    let changes = [
        Change::Set {
            object,
            field: 0,
            value: Vec::new(),
        },
        Change::SetIf {
            object,
            field: 0,
            expected: Vec::new(),
            value: Vec::new(),
        },
        Change::Add {
            object,
            field: 0,
            by: 0,
        },
        Change::Insert {
            place,
            anchor: Anchor::End,
            objects: vec![(object, Object::new(None, Vec::new()))],
        },
        Change::Put {
            object,
            field: 0,
            key: Vec::new(),
            value: None,
        },
        Change::PutIf {
            object,
            field: 0,
            key: Vec::new(),
            expected: None,
            value: None,
        },
        Change::Paint {
            object,
            field: 0,
            cells: Vec::new(),
        },
        Change::Reshape {
            object,
            field: 0,
            expected: None,
            bounds: Bounds::default(),
            cells: Vec::new(),
        },
        Change::Remove { object },
        Change::RemoveIf {
            object,
            expected: Vec::new(),
        },
        Change::Move {
            object,
            place,
            anchor: Anchor::Start,
        },
        Change::MoveIf {
            object,
            expected: place,
            place,
            anchor: Anchor::Start,
        },
        Change::Stamp {
            object,
            field: 0,
            key: Vec::new(),
            stamped: Stamped {
                stamp: Stamp::default(),
                value: None,
            },
        },
    ];
    for (position, change) in changes.iter().enumerate() {
        assert_eq!(
            usize::from(tag(change)),
            position,
            "stored edits hold {change:?} at this position; add new variants at the end"
        );
    }

    let anchors = [Anchor::Start, Anchor::After(object), Anchor::End];
    for (position, anchor) in anchors.iter().enumerate() {
        assert_eq!(usize::from(tag(anchor)), position);
    }
}
