use super::*;

#[test]
fn a_node_cannot_move_inside_itself() {
    let mut outline = Document::<Node>::default();
    let (parent, add_parent) = Node::CHILDREN.insert(ObjectId::ROOT, Anchor::End, &Node::default());
    let (child, add_child) = Node::CHILDREN.insert(parent, Anchor::End, &Node::default());
    outline.apply(&Edit(vec![add_parent, add_child]));
    let before = outline.clone();

    outline.apply(&Node::CHILDREN.move_into(child, Anchor::End, parent).into());
    outline.apply(&Change::remove(ObjectId::ROOT).into());

    assert_eq!(outline, before);
    assert_eq!(
        outline.step(&Node::CHILDREN.move_into(child, Anchor::End, parent).into()),
        None
    );
}
