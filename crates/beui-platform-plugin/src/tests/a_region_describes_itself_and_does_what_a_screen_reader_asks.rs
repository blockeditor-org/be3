use super::*;
use beui_core::accesskit::{Action, NodeId as AccessNodeId, Role, TreeId};

fn button(tree: &TreeUpdate) -> Option<AccessNodeId> {
    tree.nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Button && node.label() == Some("Save"))
        .map(|(id, _)| *id)
}

#[test]
fn a_region_describes_itself_and_does_what_a_screen_reader_asks() {
    let _root = RootScope::new();
    let clicks = Rc::new(Cell::new(0));
    let counted = Rc::clone(&clicks);
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(move |_| {
            let counted = Rc::clone(&counted);
            view! {
                <styled::Button
                    variant=styled::ButtonVariant::Primary
                    label="Save"
                    on_click={move || counted.set(counted.get() + 1)}
                />
            }
        }),
    );
    let mut described = region(vec2(100.0, 60.0), vec2(10.0, 20.0), [90, 40]);
    described.describe = Some(1);

    let tree = one(&mut surface, &described)
        .description
        .and_then(|description| description.tree)
        .expect("the first described frame carries the whole tree");
    let root = tree
        .tree
        .as_ref()
        .expect("a whole tree names its root")
        .root;
    let transform = tree
        .nodes
        .iter()
        .find(|(id, _)| *id == root)
        .and_then(|(_, node)| node.transform().copied());
    assert_eq!(
        transform,
        Some(Affine::scale(1.0)),
        "the tree is in the region's points, from the region's corner, the corner of its window"
    );
    let save = button(&tree).expect("the button is described");

    let again = one(&mut surface, &described)
        .description
        .and_then(|description| description.tree);
    assert!(
        again.as_ref().is_none_or(|tree| button(tree).is_none()),
        "an unchanged button is not sent again"
    );

    surface.node_action(
        &described,
        ActionRequest {
            action: Action::Click,
            target_tree: TreeId::ROOT,
            target_node: save,
            data: None,
        },
    );
    one(&mut surface, &described);
    assert_eq!(clicks.get(), 1);

    described.describe = None;
    assert!(one(&mut surface, &described).description.is_none());
    described.describe = Some(2);
    let tree = one(&mut surface, &described)
        .description
        .and_then(|description| description.tree)
        .expect("describing again sends the whole tree again");
    assert_eq!(button(&tree), Some(save));
}
