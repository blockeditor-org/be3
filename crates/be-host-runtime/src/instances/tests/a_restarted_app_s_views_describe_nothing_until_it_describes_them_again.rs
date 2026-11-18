use super::*;
use be_sdk_api::accesskit::{Node, NodeId, Role, Tree, TreeId, TreeUpdate};
use be_sdk_app_host::accessibility::Placing;

fn describe(instances: &mut Instances) {
    let screen = instances
        .screen_id(INSTANCE, REGION)
        .expect("the view has a screen");
    let mut root = Node::new(Role::Window);
    root.set_children(vec![NodeId(2)]);
    let mut button = Node::new(Role::Button);
    button.set_label("Save");
    instances.set_frame_reports(vec![be_sdk_api::FrameReport {
        description: Some(
            be_sdk_api::Description {
                tree: Some(TreeUpdate {
                    nodes: vec![(NodeId(1), root), (NodeId(2), button)],
                    tree: Some(Tree::new(NodeId(1))),
                    tree_id: TreeId::ROOT,
                    focus: NodeId(2),
                }),
            }
            .into(),
        ),
        actions: Some(vec![be_sdk_api::DescribedAction {
            id: "save".to_owned(),
            label: "Save".to_owned(),
            glyph: None,
            shortcut: None,
            enabled: true,
            checked: None,
            live: true,
        }]),
        ..frame_report(screen)
    }]);
}

/// The labels of the view's tree as the host grafts it, and of the actions it offers.
fn labels(instances: &mut Instances) -> (Vec<String>, Vec<String>) {
    let described = instances
        .described(INSTANCE, REGION)
        .expect("the view was described");
    let actions = described
        .actions
        .iter()
        .map(|action| action.label.clone())
        .collect();
    let grafted = described.tree.grafted(
        Placing {
            title: Some("App"),
            area: Rect::from_min_size(pos2(0.0, 0.0), vec2(100.0, 100.0)),
            origin: Vec2::ZERO,
            scale: 1.0,
        },
        true,
        true,
    );
    let tree = grafted
        .nodes
        .iter()
        .filter_map(|(_, node)| node.label().map(str::to_owned))
        .collect();
    (tree, actions)
}

/// The tree and actions an app described are of the run that drew the view: once it restarts,
/// the view's pane is empty and it offers no actions until the new run describes it.
#[test]
fn a_restarted_app_s_views_describe_nothing_until_it_describes_them_again() {
    let mut instances = placed();
    instances.next_screens(PASS);
    describe(&mut instances);
    let described = || (vec!["App".to_owned(), "Save".to_owned()], vec!["Save".to_owned()]);
    assert_eq!(labels(&mut instances), described());

    instances.reopen();
    assert_eq!(labels(&mut instances), (vec!["App".to_owned()], Vec::new()));

    describe(&mut instances);
    assert_eq!(labels(&mut instances), described());
}
