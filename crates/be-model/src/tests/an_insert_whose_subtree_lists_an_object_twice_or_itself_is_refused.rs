use super::*;

use crate::{Items, Object, Place, Value};

fn node(parent: Place, children: &[ObjectId]) -> Object {
    let mut fields = Node::blank();
    fields[1] = Value::List(Items::from_ids(children.iter().copied()));
    Object::new(Some(parent), fields)
}

#[test]
fn an_insert_whose_subtree_lists_an_object_twice_or_itself_is_refused() {
    let root = Node::CHILDREN.of(ObjectId::ROOT);
    let (top, left, right, shared) = (
        ObjectId::new(),
        ObjectId::new(),
        ObjectId::new(),
        ObjectId::new(),
    );
    let twice = Change::Insert {
        place: root,
        anchor: Anchor::End,
        client: crate::local_client(),
        objects: vec![
            (top, node(root, &[left, right])),
            (left, node(Node::CHILDREN.of(top), &[shared])),
            (right, node(Node::CHILDREN.of(top), &[shared])),
            (shared, node(Node::CHILDREN.of(left), &[])),
        ],
    };
    let itself = Change::Insert {
        place: root,
        anchor: Anchor::End,
        client: crate::local_client(),
        objects: vec![
            (top, node(root, &[left])),
            (left, node(Node::CHILDREN.of(top), &[top])),
        ],
    };
    let empty = Document::<Node>::default();

    for change in [twice, itself] {
        let mut document = empty.clone();
        document.apply(&change.into());
        assert_eq!(document, empty);
    }
}
