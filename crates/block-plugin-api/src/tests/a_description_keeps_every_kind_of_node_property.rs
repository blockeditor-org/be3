use super::*;
use accesskit::{
    Action, Affine, Color, CustomAction, Node, NodeId, Rect as NodeRect, Role, TextDecoration,
    TextDecorationStyle, TextPosition, TextSelection, Toggled, Tree, TreeId, TreeUpdate,
};

#[test]
fn a_description_keeps_every_kind_of_node_property() {
    let mut button = Node::new(Role::Button);
    button.set_label("Save");
    button.add_action(Action::Click);
    button.add_action(Action::Focus);
    button.set_bounds(NodeRect {
        x0: 1.0,
        y0: 2.0,
        x1: 30.0,
        y1: 40.5,
    });
    button.set_toggled(Toggled::Mixed);
    button.set_disabled();
    button.set_transform(Affine::scale(2.0));
    button.set_custom_actions(vec![CustomAction {
        id: 3,
        description: "Pin".into(),
    }]);
    let mut text = Node::new(Role::TextInput);
    text.set_value("Buy bread");
    text.set_numeric_value(4.5);
    text.set_font_size(13.0);
    text.set_row_count(7);
    text.set_character_lengths([1, 2, 3]);
    text.set_character_positions([0.0, 4.5]);
    text.set_expanded(true);
    text.set_underline(TextDecoration {
        style: TextDecorationStyle::Wavy,
        color: Color {
            red: 1,
            green: 2,
            blue: 3,
            alpha: 4,
        },
    });
    text.set_text_selection(TextSelection {
        anchor: TextPosition {
            node: NodeId(2),
            character_index: 1,
        },
        focus: TextPosition {
            node: NodeId(2),
            character_index: 4,
        },
    });
    text.set_active_descendant(NodeId(1));
    let mut root = Node::new(Role::Window);
    root.set_children(vec![NodeId(1), NodeId(2)]);
    let description = Description {
        tree: Some(TreeUpdate {
            nodes: vec![(NodeId(0), root), (NodeId(1), button), (NodeId(2), text)],
            tree: Some(Tree {
                root: NodeId(0),
                toolkit_name: Some("beui".into()),
                toolkit_version: None,
            }),
            tree_id: TreeId::ROOT,
            focus: NodeId(2),
        }),
        actions: vec![DescribedAction {
            id: "editor.undo".into(),
            label: "Undo".into(),
            shortcut: Some("Ctrl+Z".into()),
            enabled: true,
            checked: None,
            live: false,
        }],
    };
    let carried = crate::ops::encode(&crate::accessibility::Description::from(
        description.clone(),
    ));
    let decoded: Description = crate::ops::decode::<crate::accessibility::Description>(&carried)
        .expect("the description decodes")
        .into();
    assert_eq!(
        format!("{decoded:?}"),
        format!("{description:?}"),
        "a description travels as the accessibility interface's tree and comes back as AccessKit's"
    );
    assert_eq!(
        crate::ops::encode(&crate::accessibility::Description::from(decoded)),
        carried
    );
}
