use super::*;
use accesskit::{Action, ActionData, ActionRequest, Node, NodeId, Role, TreeId, TreeUpdate};

#[test]
fn a_screen_readers_action_round_trips_and_a_huge_tree_is_refused() {
    let action = Message::NodeAction {
        screen: ScreenId(4),
        request: ActionRequest {
            action: Action::SetValue,
            target_tree: TreeId::ROOT,
            target_node: NodeId(1 << 41 | 7),
            data: Some(ActionData::Value("Buy bread".into())),
        },
    };
    assert_eq!(
        decode_frame(&encode_frame(&action).unwrap()).unwrap(),
        action
    );

    let huge = Message::Frames(vec![FrameReport {
        screen: ScreenId(4),
        content: ChildRect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        painted: Vec::new(),
        floating: Vec::new(),
        drawing: Vec::new(),
        claims: Vec::new(),
        description: Some(Description {
            tree: Some(TreeUpdate {
                nodes: (0..=MAX_ACCESSIBILITY_NODES as u64)
                    .map(|id| (NodeId(id), Node::new(Role::Label)))
                    .collect(),
                tree: None,
                tree_id: TreeId::ROOT,
                focus: NodeId(0),
            }),
            actions: Vec::new(),
        }),
    }]);
    assert_eq!(
        encode_frame(&huge),
        Err(DecodeError::LimitExceeded("accessibility nodes"))
    );
}
