use super::*;

fn names(node: &Node, out: &mut Vec<String>) {
    for child in node.children.iter() {
        out.push(child.name.clone());
        names(child, out);
    }
}

#[test]
fn nodes_moved_into_each_other_on_each_side_merge_without_a_cycle() {
    let node = |name: &str| Node {
        name: name.to_owned(),
        children: List::default(),
    };
    let (first, add_first) = Node::CHILDREN.insert(ObjectId::ROOT, Anchor::End, &node("first"));
    let (second, add_second) = Node::CHILDREN.insert(ObjectId::ROOT, Anchor::End, &node("second"));
    let mut base = Document::<Node>::default();
    base.apply(&Edit(vec![add_first, add_second]));
    let mut ours = base.clone();
    ours.apply(&Node::CHILDREN.move_into(second, Anchor::End, first).into());
    let mut theirs = base.clone();
    theirs.apply(&Node::CHILDREN.move_into(first, Anchor::End, second).into());

    let (merged, conflicts) = Document::merge(&base, &ours, &theirs);

    assert!(conflicts >= 1);
    let mut reachable = Vec::new();
    names(&merged.root(), &mut reachable);
    reachable.sort();
    assert_eq!(reachable, ["first", "second"]);
    assert_eq!(
        Document::<Node>::from_bytes(&merged.to_bytes()).expect("the merge decodes"),
        merged
    );
}
