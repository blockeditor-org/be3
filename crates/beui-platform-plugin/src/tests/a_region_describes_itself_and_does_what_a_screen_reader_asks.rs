use super::*;
use beui::reactive::*;
use beui::styled;
use beui_core::accesskit::{Action, NodeId, Role, TreeId};
use beui_core::document::Document;

struct Shown(Document);

impl App for Shown {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.0.show(context, rect);
    }
}

fn button(tree: &TreeUpdate) -> Option<NodeId> {
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
    let document = build(move || {
        view! {
            <styled::Root>
                <styled::Button
                    variant=styled::ButtonVariant::Primary
                    label="Save"
                    on_click={move || counted.set(counted.get() + 1)}
                />
            </styled::Root>
        }
    });
    let mut surface =
        PluginSurface::launch(EditorHost::default(), beui::context(), Shown(document));
    let mut described = region(
        Rect::from_min_size(pos2(10.0, 20.0), vec2(100.0, 60.0)),
        [120, 80],
    );
    described.describe = Some(1);

    let tree = surface
        .update(&described)
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
        Some(Affine::translate((-10.0, -20.0))),
        "the tree is in the region's points, from the region's corner"
    );
    let save = button(&tree).expect("the button is described");

    let again = surface
        .update(&described)
        .description
        .and_then(|description| description.tree);
    assert!(
        again.as_ref().is_none_or(|tree| button(tree).is_none()),
        "an unchanged button is not sent again"
    );

    surface.node_action(ActionRequest {
        action: Action::Click,
        target_tree: TreeId::ROOT,
        target_node: save,
        data: None,
    });
    surface.update(&described);
    assert_eq!(clicks.get(), 1);

    described.describe = None;
    assert!(surface.update(&described).description.is_none());
    described.describe = Some(2);
    let tree = surface
        .update(&described)
        .description
        .and_then(|description| description.tree)
        .expect("describing again sends the whole tree again");
    assert_eq!(button(&tree), Some(save));
}
